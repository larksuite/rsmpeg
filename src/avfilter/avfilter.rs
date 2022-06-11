use std::{
    ffi::CStr,
    mem::size_of,
    ops::Drop,
    ptr::{self, NonNull},
};

use crate::{
    avutil::AVFrame,
    error::{Result, RsmpegError},
    ffi,
    shared::*,
};

wrap_ref!(AVFilter: ffi::AVFilter);

impl AVFilter {
    /// Get a filter definition matching the given name.
    pub fn get_by_name(filter_name: &CStr) -> Result<AVFilterRef<'static>> {
        let filter = unsafe { ffi::avfilter_get_by_name(filter_name.as_ptr()) }
            .upgrade()
            .ok_or(RsmpegError::FilterNotFound)?;
        Ok(unsafe { AVFilterRef::from_raw(filter) })
    }
}

impl Drop for AVFilter {
    fn drop(&mut self) {
        // Do nothing, filter is always static
    }
}

wrap_mut!(AVFilterContext: ffi::AVFilterContext);

impl AVFilterContext {
    /// Set property of a [`AVFilterContext`].
    pub fn set_property<U>(&mut self, key: &CStr, value: &U) -> Result<()> {
        unsafe {
            ffi::av_opt_set_bin(
                self.as_mut_ptr().cast(),
                key.as_ptr(),
                value as *const _ as *const u8,
                size_of::<U>() as i32,
                ffi::AV_OPT_SEARCH_CHILDREN as i32,
            )
        }
        .upgrade()
        .map_err(RsmpegError::SetPropertyError)?;
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
            .upgrade()
            .map_err(RsmpegError::BufferSrcAddFrameError)?;
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
    pub fn get_filter(&mut self, name: &CStr) -> Option<AVFilterContextMut> {
        unsafe {
            ffi::avfilter_graph_get_filter(self.as_mut_ptr(), name.as_ptr())
                .upgrade()
                .map(|raw| AVFilterContextMut::from_raw(raw))
        }
    }
}

impl<'graph> AVFilterGraph {
    /// Create and add a [`AVFilter`] instance into an existing
    /// [`AVFilterGraph`]. The filter instance is created from the `filter` and
    /// inited with the parameter `args`.
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
        .upgrade()
        .map_err(RsmpegError::CreateFilterError)?;

        let filter_context = NonNull::new(filter_context).unwrap();

        Ok(unsafe { AVFilterContextMut::from_raw(filter_context) })
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
