use anyhow::{bail, Context as AnyhowContext, Result};
use cstr::cstr;
use once_cell::sync::Lazy as SyncLazy;
use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext},
    avformat::{AVFormatContextInput, AVFormatContextOutput},
    avutil::{av_get_default_channel_layout, ra, AVAudioFifo, AVFrame, AVSamples},
    error::RsmpegError,
    ffi,
    swresample::SwrContext,
};
use std::{ffi::CStr, sync::Mutex};

fn open_input_file(input_file: &CStr) -> Result<(AVFormatContextInput, AVCodecContext, usize)> {
    let input_format_context =
        AVFormatContextInput::open(input_file).context("Failed to get encoder")?;
    let (audio_index, decoder) = input_format_context
        .find_best_stream(ffi::AVMediaType_AVMEDIA_TYPE_AUDIO)?
        .context("Failed to find audio stream")?;

    let mut decode_context = AVCodecContext::new(&decoder);
    decode_context.apply_codecpar(
        &input_format_context
            .streams()
            .get(audio_index)
            .unwrap()
            .codecpar(),
    )?;
    decode_context.open(None)?;
    Ok((input_format_context, decode_context, audio_index))
}

fn open_output_file(
    output_file: &CStr,
    decode_context: &AVCodecContext,
) -> Result<(AVFormatContextOutput, AVCodecContext)> {
    // Create a new format context for the output container format.
    let mut output_format_context =
        AVFormatContextOutput::create(output_file, None).context("Failed to open output file.")?;

    // Find the encoder to be used by its name.
    let encode_codec = AVCodec::find_encoder(ffi::AVCodecID_AV_CODEC_ID_AAC)
        .context("Failed to find aac encoder")?;

    let mut encode_context = AVCodecContext::new(&encode_codec);

    const OUTPUT_CHANNELS: i32 = 2;
    const OUTPUT_BIT_RATE: i64 = 96000;
    // Set the basic encoder parameters.
    // The input file's sample rate is used to avoid a sample rate conversion.
    encode_context.set_channels(OUTPUT_CHANNELS);
    encode_context.set_channel_layout(av_get_default_channel_layout(OUTPUT_CHANNELS));
    encode_context.set_sample_rate(decode_context.sample_rate);
    encode_context.set_sample_fmt(encode_codec.sample_fmts().unwrap()[0]);
    encode_context.set_bit_rate(OUTPUT_BIT_RATE);

    // Allow the use of the experimental AAC encoder.
    encode_context.set_strict_std_compliance(ffi::FF_COMPLIANCE_EXPERIMENTAL);

    // Open the encoder for the audio stream to use it later.
    encode_context.open(None)?;

    {
        // Create a new audio stream in the output file container.
        let mut stream = output_format_context.new_stream();
        stream.set_codecpar(encode_context.extract_codecpar());
        // Set the sample rate for the container.
        stream.set_time_base(ra(decode_context.sample_rate, 1));
    }

    Ok((output_format_context, encode_context))
}

fn init_resampler(
    decode_context: &mut AVCodecContext,
    encode_context: &mut AVCodecContext,
) -> Result<SwrContext> {
    let mut resample_context = SwrContext::new(
        av_get_default_channel_layout(encode_context.channels),
        encode_context.sample_fmt,
        encode_context.sample_rate,
        av_get_default_channel_layout(decode_context.channels),
        decode_context.sample_fmt,
        decode_context.sample_rate,
    )
    .context("Swrcontext parameters incorrect.")?;
    resample_context.init()?;
    Ok(resample_context)
}

fn add_samples_to_fifo(
    fifo: &mut AVAudioFifo,
    samples_buffer: &AVSamples,
    num_samples: i32,
) -> Result<()> {
    fifo.realloc(fifo.size() + num_samples);
    if unsafe { fifo.write(samples_buffer.audio_data.as_ptr(), num_samples) }? < num_samples {
        bail!("samples doesn't all written.");
    }
    Ok(())
}

fn create_output_frame(
    nb_samples: i32,
    channel_layout: u64,
    sample_fmt: i32,
    sample_rate: i32,
) -> AVFrame {
    let mut frame = AVFrame::new();
    frame.set_nb_samples(nb_samples);
    frame.set_channel_layout(channel_layout);
    frame.set_format(sample_fmt);
    frame.set_sample_rate(sample_rate);

    frame.alloc_buffer().unwrap();

    frame
}

/// Return boolean: if data is written.
fn encode_audio_frame(
    mut frame: Option<AVFrame>,
    output_format_context: &mut AVFormatContextOutput,
    encode_context: &mut AVCodecContext,
) -> Result<()> {
    static PTS: SyncLazy<Mutex<i64>> = SyncLazy::new(|| Mutex::new(0));

    if let Some(frame) = frame.as_mut() {
        let mut pts = PTS.lock().unwrap();
        frame.set_pts(*pts);
        *pts += frame.nb_samples as i64;
    }

    encode_context.send_frame(frame.as_ref())?;
    loop {
        let mut packet = match encode_context.receive_packet() {
            Ok(packet) => packet,
            Err(RsmpegError::EncoderDrainError) | Err(RsmpegError::EncoderFlushedError) => {
                break;
            }
            Err(e) => bail!(e),
        };

        output_format_context.write_frame(&mut packet)?;
    }
    Ok(())
}

fn load_encode_and_write(
    fifo: &mut AVAudioFifo,
    output_format_context: &mut AVFormatContextOutput,
    encode_context: &mut AVCodecContext,
) -> Result<()> {
    let nb_samples = fifo.size().min(encode_context.frame_size);
    let mut frame = create_output_frame(
        nb_samples,
        encode_context.channel_layout,
        encode_context.sample_fmt,
        encode_context.sample_rate,
    );
    if unsafe { fifo.read(frame.data_mut().as_mut_ptr(), nb_samples)? } < nb_samples {
        bail!("samples doesn't all read");
    }
    encode_audio_frame(Some(frame), output_format_context, encode_context)?;
    Ok(())
}

fn transcode_aac(input_file: &CStr, output_file: &CStr) -> Result<()> {
    // Open the input file for reading.
    let (mut input_format_context, mut decode_context, audio_stream_index) =
        open_input_file(input_file)?;

    // Open the output file for writing.
    let (mut output_format_context, mut encode_context) =
        open_output_file(output_file, &mut decode_context)?;

    // Initialize the resampler to be able to convert audio sample formats.
    let resample_context = init_resampler(&mut decode_context, &mut encode_context)?;

    // Initialize the FIFO buffer to store audio samples to be encoded.
    let mut fifo = AVAudioFifo::new(encode_context.sample_fmt, encode_context.channels, 1);

    // Write the header of the output file container.
    output_format_context.write_header(&mut None)?;

    let output_nb_sample = encode_context.frame_size;

    // Loop as long as we have input samples to read or output samples to write;
    // abort as soon as we have neither.
    loop {
        loop {
            // We get enough audio samples.
            if fifo.size() >= output_nb_sample {
                break;
            }

            // Break when no more input packets.
            let packet = match input_format_context.read_packet()? {
                Some(x) => x,
                None => break,
            };

            // Ignore non audio stream packets.
            if packet.stream_index as usize != audio_stream_index {
                continue;
            }

            decode_context.send_packet(Some(&packet))?;

            loop {
                let frame = match decode_context.receive_frame() {
                    Ok(frame) => frame,
                    Err(RsmpegError::DecoderDrainError) | Err(RsmpegError::DecoderFlushedError) => {
                        break;
                    }
                    Err(e) => bail!(e),
                };

                let mut output_samples = AVSamples::new(
                    encode_context.channels,
                    frame.nb_samples,
                    encode_context.sample_fmt,
                    0,
                )
                .context("Create samples buffer failed.")?;

                unsafe {
                    resample_context.convert(
                        &mut output_samples,
                        frame.extended_data as *const _,
                        frame.nb_samples,
                    )?;
                }

                add_samples_to_fifo(&mut fifo, &output_samples, frame.nb_samples)?;
            }
        }

        // If we still cannot get enough samples, break.
        if fifo.size() < output_nb_sample {
            break;
        }

        // Write frame as much as possible.
        while fifo.size() >= output_nb_sample {
            load_encode_and_write(&mut fifo, &mut output_format_context, &mut encode_context)?;
        }
    }

    // Flush encode context
    encode_audio_frame(None, &mut output_format_context, &mut encode_context)?;

    output_format_context.write_trailer().unwrap();
    Ok(())
}

#[test]
fn transcode_aac_test0() {
    std::fs::create_dir_all("tests/output/transcode_aac/").unwrap();
    transcode_aac(
        cstr!("tests/assets/audios/sample1_short.aac"),
        cstr!("tests/output/transcode_aac/output_short.aac"),
    )
    .unwrap();
}

#[test]
fn transcode_aac_test1() {
    std::fs::create_dir_all("tests/output/transcode_aac/").unwrap();
    transcode_aac(
        cstr!("tests/assets/vids/big_buck_bunny.mp4"),
        cstr!("tests/output/transcode_aac/big_buck_bunny.aac"),
    )
    .unwrap();
}
