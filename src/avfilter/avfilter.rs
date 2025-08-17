use std::{
    ffi::CStr,
    mem::{size_of, MaybeUninit},
    ptr::{self, NonNull},
};

use crate::{
    avutil::{AVChannelLayout, AVDictionary, AVFrame},
    error::{Result, RsmpegError},
    ffi,
    shared::*,
};

wrap_ref!(AVFilter: ffi::AVFilter);

impl AVFilter {
    /// Get a filter definition matching the given name.
    pub fn get_by_name(filter_name: &CStr) -> Option<AVFilterRef<'static>> {
        let filter = unsafe { ffi::avfilter_get_by_name(filter_name.as_ptr()) }.upgrade()?;
        Some(unsafe { AVFilterRef::from_raw(filter) })
    }
}

impl Drop for AVFilter {
    fn drop(&mut self) {
        // Do nothing, filter is always static
    }
}

wrap_mut!(AVFilterContext: ffi::AVFilterContext);

impl AVFilterContext {
    /// Initialize a filter with the supplied dictionary of options.
    pub fn init_dict(&mut self, options: &mut Option<AVDictionary>) -> Result<()> {
        let mut options_ptr = options
            .as_mut()
            .map(|x| x.as_mut_ptr())
            .unwrap_or_else(std::ptr::null_mut);

        unsafe { ffi::avfilter_init_dict(self.as_mut_ptr(), &mut options_ptr) }.upgrade()?;

        // Forget the old options since it's ownership is transferred.
        let mut new_options = options_ptr
            .upgrade()
            .map(|x| unsafe { AVDictionary::from_raw(x) });
        std::mem::swap(options, &mut new_options);
        new_options.map(|x| x.into_raw());

        Ok(())
    }

    /// Initialize a filter with the supplied parameters.
    /// - @param args Options to initialize the filter with. This must be a
    ///   ':'-separated list of options in the 'key=value' form.
    ///   May be NULL if the options have been set directly using the
    ///   AVOptions API or there are no options that need to be set.
    pub fn init_str(&mut self, args: Option<&CStr>) -> Result<()> {
        unsafe {
            ffi::avfilter_init_str(
                self.as_mut_ptr(),
                args.map(|x| x.as_ptr()).unwrap_or_else(ptr::null),
            )
        }
        .upgrade()?;
        Ok(())
    }

    /// Set property of a [`AVFilterContext`].
    pub fn opt_set_bin<U>(&mut self, key: &CStr, value: &U) -> Result<()> {
        unsafe {
            ffi::av_opt_set_bin(
                self.as_mut_ptr().cast(),
                key.as_ptr(),
                value as *const _ as *const u8,
                size_of::<U>() as i32,
                ffi::AV_OPT_SEARCH_CHILDREN as i32,
            )
        }
        .upgrade()?;
        Ok(())
    }

    /// Set property of a [`AVFilterContext`].
    pub fn opt_set(&mut self, key: &CStr, value: &CStr) -> Result<()> {
        unsafe {
            ffi::av_opt_set(
                self.as_mut_ptr().cast(),
                key.as_ptr(),
                value.as_ptr(),
                ffi::AV_OPT_SEARCH_CHILDREN as i32,
            )
        }
        .upgrade()?;
        Ok(())
    }

    /// Set integer property of a [`AVFilterContext`].
    pub fn opt_set_int(&mut self, key: &CStr, value: i64) -> Result<()> {
        unsafe {
            ffi::av_opt_set_int(
                self.as_mut_ptr().cast(),
                key.as_ptr(),
                value,
                ffi::AV_OPT_SEARCH_CHILDREN as i32,
            )
        }
        .upgrade()?;
        Ok(())
    }

    /// Set double property of a [`AVFilterContext`].
    pub fn opt_set_double(&mut self, key: &CStr, value: f64) -> Result<()> {
        unsafe {
            ffi::av_opt_set_double(
                self.as_mut_ptr().cast(),
                key.as_ptr(),
                value,
                ffi::AV_OPT_SEARCH_CHILDREN as i32,
            )
        }
        .upgrade()?;
        Ok(())
    }

    /// Set AVRational property of a [`AVFilterContext`].
    pub fn opt_set_q(&mut self, key: &CStr, value: ffi::AVRational) -> Result<()> {
        unsafe {
            ffi::av_opt_set_q(
                self.as_mut_ptr().cast(),
                key.as_ptr(),
                value,
                ffi::AV_OPT_SEARCH_CHILDREN as i32,
            )
        }
        .upgrade()?;
        Ok(())
    }

    /// Add, replace, or remove elements for an array option.
    ///
    /// This is a safe wrapper around `av_opt_set_array`.
    /// - key: option name
    /// - start_elem: index of the first array element to modify
    /// - vals: None to remove elements, Some(&[T]) to insert/replace elements
    /// - val_type: the `AVOptionType` corresponding to T (e.g. AV_OPT_TYPE_INT)
    ///
    /// Note: This wrapper always searches children (AV_OPT_SEARCH_CHILDREN).
    #[cfg(feature = "ffmpeg7_1")]
    pub fn opt_set_array<T>(
        &mut self,
        key: &CStr,
        start_elem: u32,
        vals: Option<&[T]>,
        val_type: ffi::AVOptionType,
    ) -> Result<()> {
        let (nb_elems, val_ptr) = match vals {
            Some(slice) => (
                slice.len() as u32,
                slice.as_ptr() as *const std::os::raw::c_void,
            ),
            None => (0u32, std::ptr::null()),
        };
        unsafe {
            ffi::av_opt_set_array(
                self.as_mut_ptr().cast(),
                key.as_ptr(),
                ffi::AV_OPT_SEARCH_CHILDREN as i32,
                start_elem,
                nb_elems,
                val_type,
                val_ptr,
            )
        }
        .upgrade()?;
        Ok(())
    }

    /// Add a frame to the buffer source.
    pub fn buffersrc_add_frame(
        &mut self,
        mut frame: Option<AVFrame>,
        flags: Option<i32>,
    ) -> Result<()> {
        let frame_ptr = match frame.as_mut() {
            Some(frame) => frame.as_mut_ptr(),
            None => ptr::null_mut(),
        };
        // `av_buffersrc_add_frame(...)` just calls
        // `av_buffersrc_add_frame_flags(..., 0)`, so this is legal.
        let flags = flags.unwrap_or(0);

        unsafe { ffi::av_buffersrc_add_frame_flags(self.as_mut_ptr(), frame_ptr, flags) }
            .upgrade()?;
        Ok(())
    }

    pub fn buffersink_get_frame(&mut self, flags: Option<i32>) -> Result<AVFrame> {
        let mut frame = AVFrame::new();
        // `av_buffersink_get_frame(...)` just calls
        // `av_buffersink_get_frame_flags(..., 0)`, so this is legal.
        let flags = flags.unwrap_or(0);
        match unsafe {
            ffi::av_buffersink_get_frame_flags(self.as_mut_ptr(), frame.as_mut_ptr(), flags)
        }
        .upgrade()
        {
            Ok(_) => Ok(frame),
            Err(AVERROR_EAGAIN) => Err(RsmpegError::BufferSinkDrainError),
            Err(ffi::AVERROR_EOF) => Err(RsmpegError::BufferSinkEofError),
            Err(err) => Err(RsmpegError::BufferSinkGetFrameError(err)),
        }
    }

    /// Set the frame size for an audio buffer sink.
    ///
    /// All calls to av_buffersink_get_buffer_ref will return a buffer with
    /// exactly the specified number of samples, or AVERROR(EAGAIN) if there is
    /// not enough. The last buffer at EOF will be padded with 0.
    pub fn buffersink_set_frame_size(&mut self, frame_size: u32) {
        unsafe { ffi::av_buffersink_set_frame_size(self.as_mut_ptr(), frame_size) }
    }

    pub fn get_type(&self) -> i32 {
        unsafe { ffi::av_buffersink_get_type(self.as_ptr()) }
    }

    pub fn get_time_base(&self) -> ffi::AVRational {
        unsafe { ffi::av_buffersink_get_time_base(self.as_ptr()) }
    }

    pub fn get_format(&self) -> i32 {
        unsafe { ffi::av_buffersink_get_format(self.as_ptr()) }
    }

    pub fn get_frame_rate(&self) -> ffi::AVRational {
        unsafe { ffi::av_buffersink_get_frame_rate(self.as_ptr()) }
    }

    pub fn get_w(&self) -> i32 {
        unsafe { ffi::av_buffersink_get_w(self.as_ptr()) }
    }

    pub fn get_h(&self) -> i32 {
        unsafe { ffi::av_buffersink_get_h(self.as_ptr()) }
    }

    pub fn get_sample_aspect_ratio(&self) -> ffi::AVRational {
        unsafe { ffi::av_buffersink_get_sample_aspect_ratio(self.as_ptr()) }
    }

    pub fn get_channels(&self) -> i32 {
        unsafe { ffi::av_buffersink_get_channels(self.as_ptr()) }
    }

    pub fn get_ch_layout(&self) -> AVChannelLayout {
        let mut ch_layout = MaybeUninit::<ffi::AVChannelLayout>::uninit();
        unsafe { ffi::av_buffersink_get_ch_layout(self.as_ptr(), ch_layout.as_mut_ptr()) }
            .upgrade()
            .unwrap();
        let ch_layout = Box::leak(Box::new(unsafe { ch_layout.assume_init() }));
        unsafe { AVChannelLayout::from_raw(NonNull::new(ch_layout).unwrap()) }
    }

    pub fn get_sample_rate(&self) -> i32 {
        unsafe { ffi::av_buffersink_get_sample_rate(self.as_ptr()) }
    }

    /// Link this filter's output pad to another filter's input pad.
    pub fn link(&mut self, srcpad: u32, dst: &mut AVFilterContext, dstpad: u32) -> Result<()> {
        unsafe { ffi::avfilter_link(self.as_mut_ptr(), srcpad, dst.as_mut_ptr(), dstpad) }
            .upgrade()?;
        Ok(())
    }
}

wrap!(AVFilterInOut: ffi::AVFilterInOut);

impl AVFilterInOut {
    /// Allocate a single, unlinked [`AVFilterInOut`] entry.
    pub fn new(name: &CStr, filter_context: &mut AVFilterContext, pad_idx: i32) -> Self {
        let name = unsafe { ffi::av_strdup(name.as_ptr()) }.upgrade().unwrap();
        let mut inout_ptr = unsafe { ffi::avfilter_inout_alloc() }.upgrade().unwrap();

        let inout_mut = unsafe { inout_ptr.as_mut() };
        inout_mut.name = name.as_ptr();
        inout_mut.filter_ctx = filter_context.as_mut_ptr();
        inout_mut.pad_idx = pad_idx;
        inout_mut.next = ptr::null_mut();

        unsafe { Self::from_raw(inout_ptr) }
    }
}

impl Drop for AVFilterInOut {
    /// This frees a linked [`AVFilterInOut`] chain.
    fn drop(&mut self) {
        let mut inout = self.as_mut_ptr();
        unsafe {
            ffi::avfilter_inout_free(&mut inout);
        }
    }
}

wrap!(AVFilterGraph: ffi::AVFilterGraph);

impl AVFilterGraph {
    /// Allocate a filter graph.
    pub fn new() -> Self {
        let filter_graph = unsafe { ffi::avfilter_graph_alloc() }.upgrade().unwrap();

        unsafe { Self::from_raw(filter_graph) }
    }

    /// Add a graph described by a string to a [`AVFilterGraph`].
    ///
    /// This function returns the inputs and outputs (if any) that are left
    /// unlinked after parsing the graph and the caller then deals with them.
    pub fn parse_ptr(
        &self,
        filter_spec: &CStr,
        mut inputs: Option<AVFilterInOut>,
        mut outputs: Option<AVFilterInOut>,
    ) -> Result<(Option<AVFilterInOut>, Option<AVFilterInOut>)> {
        let mut inputs_new = inputs
            .as_mut()
            .map(|x| x.as_mut_ptr())
            .unwrap_or(ptr::null_mut());
        let mut outputs_new = outputs
            .as_mut()
            .map(|x| x.as_mut_ptr())
            .unwrap_or(ptr::null_mut());

        // FFmpeg `avfilter_graph_parse*`'s documentation states:
        //
        // This function makes no reference whatsoever to already existing parts
        // of the graph and the inputs parameter will on return contain inputs
        // of the newly parsed part of the graph.  Analogously the outputs
        // parameter will contain outputs of the newly created filters.
        //
        // So the function is designed to take immutable reference to the FilterGraph
        unsafe {
            ffi::avfilter_graph_parse_ptr(
                self.as_ptr() as _,
                filter_spec.as_ptr(),
                &mut inputs_new,
                &mut outputs_new,
                ptr::null_mut(),
            )
        }
        .upgrade()?;

        // If no error, inputs and outputs pointer are dangling, manually erase
        // them *without* dropping. Do this because we need to drop inputs and
        // outputs on the error path, but don't drop them on normal path.
        let _ = inputs.map(|x| x.into_raw());
        let _ = outputs.map(|x| x.into_raw());

        // ATTENTION: TODO here we didn't bind the AVFilterInOut to the lifetime of the AVFilterGraph
        let new_inputs = inputs_new
            .upgrade()
            .map(|raw| unsafe { AVFilterInOut::from_raw(raw) });
        let new_outputs = outputs_new
            .upgrade()
            .map(|raw| unsafe { AVFilterInOut::from_raw(raw) });
        Ok((new_inputs, new_outputs))
    }

    /// Check validity and configure all the links and formats in the graph.
    pub fn config(&self) -> Result<()> {
        // ATTENTION: This takes immutable reference since it doesn't delete any filter.
        unsafe { ffi::avfilter_graph_config(self.as_ptr() as *mut _, ptr::null_mut()) }
            .upgrade()?;
        Ok(())
    }

    /// Get a filter instance identified by instance name from graph.
    pub fn get_filter(&self, name: &CStr) -> Option<AVFilterContextMut<'_>> {
        unsafe {
            ffi::avfilter_graph_get_filter(self.as_ptr() as *mut _, name.as_ptr())
                .upgrade()
                .map(|raw| AVFilterContextMut::from_raw(raw))
        }
    }
}

impl<'graph> AVFilterGraph {
    /// A convenience wrapper that allocates and initializes a filter in a single
    /// step. The filter instance is created from the filter filt and inited with the
    /// parameter args.
    pub fn create_filter_context(
        &'graph self,
        filter: &AVFilter,
        name: &CStr,
        args: Option<&CStr>,
    ) -> Result<AVFilterContextMut<'graph>> {
        let args_ptr = args.map(|s| s.as_ptr()).unwrap_or(ptr::null());
        let mut filter_context = ptr::null_mut();
        unsafe {
            ffi::avfilter_graph_create_filter(
                &mut filter_context,
                filter.as_ptr(),
                name.as_ptr(),
                args_ptr,
                ptr::null_mut(),
                // ATTENTION: We restrict the API for not removing filter, then
                // we can legally add filter and take mutable reference to it in
                // a filter graph with immutable reference.
                self.as_ptr() as *mut _,
            )
        }
        .upgrade()?;

        let filter_context = NonNull::new(filter_context).unwrap();

        Ok(unsafe { AVFilterContextMut::from_raw(filter_context) })
    }

    /// Create a new filter instance in a filter graph.
    pub fn alloc_filter_context(
        &'graph self,
        filter: &AVFilter,
        name: &CStr,
    ) -> Option<AVFilterContextMut<'graph>> {
        unsafe {
            ffi::avfilter_graph_alloc_filter(
                // ATTENTION: We restrict the API for not removing filter, then
                // we can legally add filter and take mutable reference to it in
                // a filter graph with immutable reference.
                self.as_ptr() as *mut _,
                filter.as_ptr(),
                name.as_ptr(),
            )
        }
        .upgrade()
        .map(|filter_context| unsafe { AVFilterContextMut::from_raw(filter_context) })
    }
}

impl Default for AVFilterGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AVFilterGraph {
    fn drop(&mut self) {
        let mut filter_graph = self.as_mut_ptr();
        unsafe {
            ffi::avfilter_graph_free(&mut filter_graph);
        }
    }
}
