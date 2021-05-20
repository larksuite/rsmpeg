use cstr::cstr;
use once_cell::sync::Lazy as SyncLazy;
use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext},
    avformat::{AVFormatContextInput, AVFormatContextOutput},
    avutil::{av_get_default_channel_layout, AVAudioFifo, AVFrame, AVRational, AVSamples},
    error::*,
    ffi,
    swresample::SwrContext,
};
use std::{ffi::CStr, sync::Mutex};

fn open_input_file(input_file: &CStr) -> (AVFormatContextInput, AVCodecContext) {
    let input_format_context = AVFormatContextInput::open(input_file).unwrap();
    if input_format_context.nb_streams != 1 {
        panic!("Expected one audio input stream, but multiple streams found.");
    }
    let decode_codec = AVCodec::find_decoder(
        input_format_context
            .streams()
            .get(0)
            .unwrap()
            .codecpar()
            .codec_id,
    )
    .unwrap();

    let mut decode_context = AVCodecContext::new(&decode_codec);
    decode_context
        .set_codecpar(input_format_context.streams().get(0).unwrap().codecpar())
        .unwrap();
    decode_context.open(None).unwrap();
    (input_format_context, decode_context)
}

fn open_output_file(
    output_file: &CStr,
    decode_context: &mut AVCodecContext,
) -> (AVFormatContextOutput, AVCodecContext) {
    /* Create a new format context for the output container format. */
    let mut output_format_context = AVFormatContextOutput::create(output_file).unwrap();

    /* Find the encoder to be used by its name. */
    let encode_codec = AVCodec::find_encoder(ffi::AVCodecID_AV_CODEC_ID_AAC).unwrap();

    let mut encode_context = AVCodecContext::new(&encode_codec);

    const OUTPUT_CHANNELS: i32 = 2;
    const OUTPUT_BIT_RATE: i64 = 96000;
    /* Set the basic encoder parameters.
     * The input file's sample rate is used to avoid a sample rate conversion. */
    encode_context.set_channels(OUTPUT_CHANNELS);
    encode_context.set_channel_layout(av_get_default_channel_layout(OUTPUT_CHANNELS));
    encode_context.set_sample_rate(decode_context.sample_rate);
    encode_context.set_sample_fmt(encode_codec.sample_fmts().unwrap()[0]);
    encode_context.set_bit_rate(OUTPUT_BIT_RATE);

    /* Allow the use of the experimental AAC encoder. */
    encode_context.set_strict_std_compliance(ffi::FF_COMPLIANCE_EXPERIMENTAL);

    /* Open the encoder for the audio stream to use it later. */
    encode_context.open(None).unwrap();

    {
        /* Create a new audio stream in the output file container. */
        let mut stream = output_format_context.new_stream(None);
        /* Set the sample rate for the container. */
        stream.set_time_base(AVRational {
            den: decode_context.sample_rate,
            num: 1,
        });
        stream.set_codecpar(encode_context.extract_codecpar());
    }

    (output_format_context, encode_context)
}

fn init_resampler(
    decode_context: &mut AVCodecContext,
    encode_context: &mut AVCodecContext,
) -> SwrContext {
    let mut resample_context = SwrContext::new(
        av_get_default_channel_layout(encode_context.channels),
        encode_context.sample_fmt,
        encode_context.sample_rate,
        av_get_default_channel_layout(decode_context.channels),
        decode_context.sample_fmt,
        decode_context.sample_rate,
    )
    .unwrap();
    /*
     * Perform a sanity check so that the number of converted samples is
     * not greater than the number of samples to be converted.
     * If the sample rates differ, this case has to be handled differently
     */
    assert!(decode_context.sample_rate == encode_context.sample_rate);
    resample_context.init().unwrap();
    resample_context
}

fn init_fifo(encode_context: &mut AVCodecContext) -> AVAudioFifo {
    AVAudioFifo::new(encode_context.sample_fmt, encode_context.channels, 1)
}

/// return (finished, frame_if_get)
fn decode_audio_frame(
    input_format_context: &mut AVFormatContextInput,
    decode_context: &mut AVCodecContext,
) -> Result<(bool, Option<AVFrame>)> {
    let packet = match input_format_context.read_packet()? {
        Some(packet) => packet,
        None => return Ok((true, None)),
    };

    decode_context.send_packet(Some(&packet))?;

    let frame = match decode_context.receive_frame() {
        Ok(frame) => frame,
        Err(RsmpegError::DecoderDrainError | RsmpegError::DecoderFlushedError) => {
            return Ok((false, None))
        }
        Err(e) => return Err(e),
    };
    return Ok((false, Some(frame)));
}

fn add_samples_to_fifo(
    fifo: &mut AVAudioFifo,
    samples_buffer: &AVSamples,
    num_samples: i32,
) -> Result<()> {
    fifo.realloc(fifo.size() + num_samples);
    if unsafe { fifo.write(samples_buffer.as_ptr(), num_samples) }? < num_samples {
        panic!("samples doesn't all written.");
    }
    Ok(())
}

/// return if finished
fn read_decode_convert_and_store(
    fifo: &mut AVAudioFifo,
    input_format_context: &mut AVFormatContextInput,
    decode_context: &mut AVCodecContext,
    encode_context: &mut AVCodecContext,
    resample_context: &mut SwrContext,
) -> Result<bool> {
    let (finished, frame) = decode_audio_frame(input_format_context, decode_context)?;
    if finished {
        return Ok(true);
    }
    if let Some(frame) = frame {
        let mut samples_buffer = AVSamples::new(
            encode_context.channels,
            frame.nb_samples,
            encode_context.sample_fmt,
            0,
        );
        unsafe {
            resample_context.convert(
                &mut samples_buffer,
                frame.extended_data as *const _,
                frame.nb_samples,
            )?;
        }
        add_samples_to_fifo(fifo, &samples_buffer, frame.nb_samples)?;
    }
    Ok(false)
}

fn init_output_frame(encode_context: &mut AVCodecContext, frame_size: i32) -> AVFrame {
    let mut frame = AVFrame::new();
    frame.set_nb_samples(frame_size);
    frame.set_channel_layout(encode_context.channel_layout);
    frame.set_format(encode_context.sample_fmt);
    frame.set_sample_rate(encode_context.sample_rate);

    frame.alloc_buffer().unwrap();

    frame
}

/// Return boolean: if data is written.
fn encode_audio_frame(
    mut frame: Option<AVFrame>,
    output_format_context: &mut AVFormatContextOutput,
    encode_context: &mut AVCodecContext,
) -> Result<bool> {
    static PTS: SyncLazy<Mutex<i64>> = SyncLazy::new(|| Mutex::new(0));

    if let Some(frame) = frame.as_mut() {
        let mut pts = PTS.lock().unwrap();
        frame.set_pts(*pts);
        *pts += frame.nb_samples as i64;
    }

    match encode_context.send_frame(frame.as_ref()) {
        Ok(_) => {}
        Err(RsmpegError::EncoderFlushedError) => return Ok(false),
        Err(e) => return Err(e),
    }
    let mut packet = match encode_context.receive_packet() {
        Ok(packet) => packet,
        Err(RsmpegError::EncoderDrainError) | Err(RsmpegError::EncoderFlushedError) => {
            return Ok(false)
        }
        Err(e) => return Err(e),
    };
    output_format_context.write_frame(&mut packet)?;
    Ok(true)
}

fn load_encode_and_write(
    fifo: &mut AVAudioFifo,
    output_format_context: &mut AVFormatContextOutput,
    encode_context: &mut AVCodecContext,
) -> Result<()> {
    let frame_size = std::cmp::min(fifo.size(), encode_context.frame_size);
    let mut frame = init_output_frame(encode_context, frame_size);
    if unsafe { fifo.read(frame.data_mut().as_mut_ptr(), frame_size)? } < frame_size {
        panic!("samples doesn't all read");
    }
    let _ = encode_audio_frame(Some(frame), output_format_context, encode_context)?;
    Ok(())
}

fn transcode_aac(input_file: &CStr, output_file: &CStr) {
    /* Open the input file for reading. */
    let (mut input_format_context, mut decode_context) = open_input_file(input_file);
    /* Open the output file for writing. */
    let (mut output_format_context, mut encode_context) =
        open_output_file(output_file, &mut decode_context);
    /* Initialize the resampler to be able to convert audio sample formats. */
    let mut resample_context = init_resampler(&mut decode_context, &mut encode_context);
    /* Initialize the FIFO buffer to store audio samples to be encoded. */
    let mut fifo = init_fifo(&mut encode_context);
    /* Write the header of the output file container. */
    output_format_context.write_header().unwrap();

    let output_frame_size = encode_context.frame_size;

    /* Loop as long as we have input samples to read or output samples
     * to write; abort as soon as we have neither. */
    loop {
        let mut finished = false;
        while fifo.size() < output_frame_size {
            finished = read_decode_convert_and_store(
                &mut fifo,
                &mut input_format_context,
                &mut decode_context,
                &mut encode_context,
                &mut resample_context,
            )
            .unwrap();
            if finished {
                break;
            }
        }

        while (finished && fifo.size() > 0) || (fifo.size() >= output_frame_size) {
            load_encode_and_write(&mut fifo, &mut output_format_context, &mut encode_context)
                .unwrap();
        }

        if finished {
            loop {
                if !encode_audio_frame(None, &mut output_format_context, &mut encode_context)
                    .unwrap()
                {
                    break;
                }
            }
            break;
        }
    }

    output_format_context.write_trailer().unwrap();
}

#[test]
fn transcode_aac_test() {
    std::fs::create_dir_all("tests/output/transcode_aac/").unwrap();
    transcode_aac(
        cstr!("tests/assets/audios/sample1.aac"),
        cstr!("tests/output/transcode_aac/output.aac"),
    );
}
