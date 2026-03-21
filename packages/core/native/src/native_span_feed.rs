use std::collections::VecDeque;

pub type CallbackFn = extern "C" fn(stream_ptr: usize, event_id: u32, arg0: usize, arg1: u64);

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrowthPolicy {
    Grow = 0,
    Block = 1,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub chunk_size: u32,
    pub initial_chunks: u32,
    pub max_bytes: u64,
    pub growth_policy: u8,
    pub auto_commit_on_full: u8,
    pub span_queue_capacity: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub bytes_written: u64,
    pub spans_committed: u64,
    pub chunks: u32,
    pub pending_spans: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SpanInfo {
    pub chunk_ptr: usize,
    pub offset: u32,
    pub len: u32,
    pub chunk_index: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ReserveInfo {
    pub ptr: usize,
    pub len: u32,
    pub reserved: u32,
}

#[derive(Debug)]
struct Chunk {
    data: Box<[u8]>,
}

impl Chunk {
    fn new(chunk_size: u32) -> Result<Self, StreamError> {
        let len = usize::try_from(chunk_size).map_err(|_| StreamError::OutOfMemory)?;
        Ok(Self {
            data: vec![0_u8; len].into_boxed_slice(),
        })
    }

    fn ptr(&self) -> *const u8 {
        self.data.as_ptr()
    }

    fn ptr_mut(&mut self) -> *mut u8 {
        self.data.as_mut_ptr()
    }

    fn len(&self) -> u32 {
        u32::try_from(self.data.len()).unwrap_or(u32::MAX)
    }
}

#[derive(Debug)]
pub struct Stream {
    options: Options,
    chunks: Vec<Chunk>,
    current_chunk_index: usize,
    write_offset: usize,
    pending_chunk_index: usize,
    pending_offset: usize,
    pending_len: usize,
    reserved_active: bool,
    reserved_chunk_index: usize,
    reserved_offset: usize,
    reserved_len: usize,
    attached: bool,
    callback: Option<CallbackFn>,
    closed: bool,
    span_ring: VecDeque<SpanInfo>,
    span_ring_capacity: u32,
    state_buffer: Vec<u8>,
    stats: Stats,
}

#[repr(u32)]
#[derive(Clone, Copy)]
enum EventId {
    ChunkAdded = 2,
    Closed = 5,
    DataAvailable = 7,
    StateBuffer = 8,
}

pub mod status {
    pub const OK: i32 = 0;
    pub const ERR_NO_SPACE: i32 = -1;
    pub const ERR_MAX_BYTES: i32 = -2;
    pub const ERR_INVALID: i32 = -3;
    pub const ERR_ALLOC: i32 = -4;
    pub const ERR_BUSY: i32 = -5;
}

const SPAN_QUEUE_CAPACITY_DEFAULT: u32 = 4096;
const NOTIFY_THRESHOLD_DEFAULT: u32 = 1;

#[derive(Debug, Eq, PartialEq)]
pub enum StreamError {
    NoSpace,
    MaxBytes,
    Invalid,
    OutOfMemory,
    Busy,
}

pub fn default_options() -> Options {
    Options {
        chunk_size: 64 * 1024,
        initial_chunks: 2,
        max_bytes: 0,
        growth_policy: GrowthPolicy::Grow as u8,
        auto_commit_on_full: 1,
        span_queue_capacity: 0,
    }
}

pub fn normalize_options(mut options: Options) -> Options {
    if options.chunk_size == 0 {
        options.chunk_size = 64 * 1024;
    }
    if options.initial_chunks == 0 {
        options.initial_chunks = 1;
    }
    if options.span_queue_capacity == 0 {
        options.span_queue_capacity = SPAN_QUEUE_CAPACITY_DEFAULT;
    }
    options
}

pub fn error_to_status(error: StreamError) -> i32 {
    match error {
        StreamError::NoSpace => status::ERR_NO_SPACE,
        StreamError::MaxBytes => status::ERR_MAX_BYTES,
        StreamError::Invalid => status::ERR_INVALID,
        StreamError::OutOfMemory => status::ERR_ALLOC,
        StreamError::Busy => status::ERR_BUSY,
    }
}

impl Stream {
    pub fn create(options: Options) -> Result<Self, StreamError> {
        let options = normalize_options(options);
        let initial_chunks =
            usize::try_from(options.initial_chunks).map_err(|_| StreamError::OutOfMemory)?;

        let mut stream = Self {
            options,
            chunks: Vec::new(),
            current_chunk_index: 0,
            write_offset: 0,
            pending_chunk_index: 0,
            pending_offset: 0,
            pending_len: 0,
            reserved_active: false,
            reserved_chunk_index: 0,
            reserved_offset: 0,
            reserved_len: 0,
            attached: false,
            callback: None,
            closed: false,
            span_ring: VecDeque::with_capacity(options.span_queue_capacity as usize),
            span_ring_capacity: options.span_queue_capacity,
            state_buffer: Vec::new(),
            stats: Stats::default(),
        };

        stream.ensure_state_capacity(stream.options.initial_chunks)?;
        for _ in 0..initial_chunks {
            stream.add_chunk_locked()?;
        }
        stream.stats.chunks = u32::try_from(stream.chunks.len()).unwrap_or(u32::MAX);
        Ok(stream)
    }

    pub fn attach(&mut self) -> Result<(), StreamError> {
        if self.closed {
            return Err(StreamError::Invalid);
        }

        self.attached = true;
        if self.callback.is_none() {
            return Ok(());
        }

        self.emit_state_buffer();
        for chunk in &self.chunks {
            self.emit_chunk_added(chunk);
        }
        if !self.span_ring.is_empty() {
            self.emit_data_available(self.span_ring.len() as u32);
        }
        Ok(())
    }

    pub fn set_callback(&mut self, callback: Option<CallbackFn>) {
        self.callback = callback;
        if self.callback.is_none() || !self.attached {
            return;
        }

        self.emit_state_buffer();
        for chunk in &self.chunks {
            self.emit_chunk_added(chunk);
        }
        if !self.span_ring.is_empty() {
            self.emit_data_available(self.span_ring.len() as u32);
        }
    }

    pub fn write(&mut self, data: &[u8]) -> Result<(), StreamError> {
        if self.closed {
            return Err(StreamError::Invalid);
        }
        if data.is_empty() {
            return Ok(());
        }
        if self.reserved_active {
            return Err(StreamError::Busy);
        }

        let mut notify = false;
        let mut remaining = data.len();
        let mut src_index = 0usize;
        let auto_commit = self.options.auto_commit_on_full != 0;
        let chunk_len =
            usize::try_from(self.options.chunk_size).map_err(|_| StreamError::OutOfMemory)?;

        while remaining > 0 {
            let mut available = chunk_len.saturating_sub(self.write_offset);
            if available == 0 {
                if self.pending_len > 0 {
                    self.commit_locked(&mut notify)?;
                }
                self.ensure_writable_chunk_locked()?;
                available = chunk_len;
            }

            if remaining > available && !auto_commit {
                return Err(StreamError::NoSpace);
            }

            let to_write = remaining.min(available);
            if self.pending_len == 0 {
                self.pending_chunk_index = self.current_chunk_index;
                self.pending_offset = self.write_offset;
            }

            let chunk = &mut self.chunks[self.current_chunk_index];
            chunk.data[self.write_offset..self.write_offset + to_write]
                .copy_from_slice(&data[src_index..src_index + to_write]);

            self.write_offset += to_write;
            self.pending_len += to_write;
            self.stats.bytes_written += u64::try_from(to_write).unwrap_or(u64::MAX);
            src_index += to_write;
            remaining -= to_write;

            if self.write_offset == chunk_len && auto_commit {
                self.commit_locked(&mut notify)?;
                if remaining > 0 {
                    self.ensure_writable_chunk_locked()?;
                }
            }
        }

        self.finish(notify, 0);
        Ok(())
    }

    pub fn reserve(&mut self, min_len: u32) -> Result<ReserveInfo, StreamError> {
        if self.closed {
            return Err(StreamError::Invalid);
        }
        self.reserve_locked(min_len)
    }

    pub fn commit_reserved(&mut self, len: u32) -> Result<(), StreamError> {
        if self.closed {
            return Err(StreamError::Invalid);
        }

        let mut notify = false;
        self.commit_reserved_locked(len, &mut notify)?;
        self.finish(notify, 0);
        Ok(())
    }

    pub fn commit(&mut self) -> Result<(), StreamError> {
        if self.closed {
            return Err(StreamError::Invalid);
        }
        if self.reserved_active {
            return Err(StreamError::Busy);
        }

        let mut notify = false;
        self.commit_locked(&mut notify)?;
        self.finish(notify, 0);
        Ok(())
    }

    pub fn get_stats(&self) -> Stats {
        self.stats
    }

    pub fn set_options(&mut self, options: Options) -> Result<(), StreamError> {
        if self.closed {
            return Err(StreamError::Invalid);
        }

        self.options.max_bytes = options.max_bytes;
        self.options.growth_policy = options.growth_policy;
        self.options.auto_commit_on_full = options.auto_commit_on_full;
        Ok(())
    }

    pub fn close(&mut self) -> Result<(), StreamError> {
        if self.closed {
            return Ok(());
        }
        if self.reserved_active {
            return Err(StreamError::Busy);
        }

        let mut notify = false;
        if self.pending_len > 0 {
            self.commit_locked(&mut notify)?;
        }
        self.closed = true;
        self.attached = false;
        self.finish(notify, 0);
        self.emit_closed();
        Ok(())
    }

    pub fn drain_spans(&mut self, out: &mut [SpanInfo]) -> u32 {
        let to_read = out.len().min(self.span_ring.len());
        for (index, slot) in out.iter_mut().take(to_read).enumerate() {
            *slot = self.span_ring[index];
        }
        for _ in 0..to_read {
            let _ = self.span_ring.pop_front();
        }
        self.stats.pending_spans = u32::try_from(self.span_ring.len()).unwrap_or(u32::MAX);
        u32::try_from(to_read).unwrap_or(u32::MAX)
    }

    fn ensure_state_capacity(&mut self, required: u32) -> Result<(), StreamError> {
        let required = usize::try_from(required).map_err(|_| StreamError::OutOfMemory)?;
        if required <= self.state_buffer.len() {
            return Ok(());
        }

        let mut new_capacity = self.state_buffer.len().max(1);
        while new_capacity < required {
            new_capacity = new_capacity.saturating_mul(2);
            if new_capacity == 0 {
                return Err(StreamError::OutOfMemory);
            }
        }

        self.state_buffer.resize(new_capacity, 0);
        if self.attached && self.callback.is_some() {
            self.emit_state_buffer();
        }
        Ok(())
    }

    fn is_chunk_free(&self, index: usize) -> bool {
        self.state_buffer.get(index).copied().unwrap_or(0) == 0
    }

    fn commit_locked(&mut self, notify: &mut bool) -> Result<(), StreamError> {
        if self.pending_len == 0 {
            return Ok(());
        }
        if self.span_ring.len() >= self.span_ring_capacity as usize {
            return Err(StreamError::NoSpace);
        }

        let chunk = &self.chunks[self.pending_chunk_index];
        let info = SpanInfo {
            chunk_ptr: chunk.ptr() as usize,
            offset: u32::try_from(self.pending_offset).map_err(|_| StreamError::OutOfMemory)?,
            len: u32::try_from(self.pending_len).map_err(|_| StreamError::OutOfMemory)?,
            chunk_index: u32::try_from(self.pending_chunk_index)
                .map_err(|_| StreamError::OutOfMemory)?,
            reserved: 0,
        };

        let queued_before = self.span_ring.len() as u32;
        self.span_ring.push_back(info);
        self.stats.pending_spans = u32::try_from(self.span_ring.len()).unwrap_or(u32::MAX);

        if let Some(state) = self.state_buffer.get_mut(self.pending_chunk_index) {
            *state = state.saturating_add(1);
            if *state == u8::MAX {
                self.write_offset = self.options.chunk_size as usize;
            }
        }

        self.stats.spans_committed += 1;
        self.pending_len = 0;
        self.pending_offset = self.write_offset;
        self.pending_chunk_index = self.current_chunk_index;

        if self.attached
            && self.callback.is_some()
            && queued_before < NOTIFY_THRESHOLD_DEFAULT
            && self.stats.pending_spans >= NOTIFY_THRESHOLD_DEFAULT
        {
            *notify = true;
        }

        Ok(())
    }

    fn reserve_locked(&mut self, min_len: u32) -> Result<ReserveInfo, StreamError> {
        if self.reserved_active || self.pending_len != 0 {
            return Err(StreamError::Busy);
        }

        self.ensure_writable_chunk_locked()?;

        let chunk = &mut self.chunks[self.current_chunk_index];
        let available = chunk.data.len().saturating_sub(self.write_offset);
        if available < min_len as usize {
            return Err(StreamError::NoSpace);
        }

        self.reserved_active = true;
        self.reserved_chunk_index = self.current_chunk_index;
        self.reserved_offset = self.write_offset;
        self.reserved_len = available;

        Ok(ReserveInfo {
            ptr: unsafe { chunk.ptr_mut().add(self.write_offset) as usize },
            len: u32::try_from(available).unwrap_or(u32::MAX),
            reserved: 0,
        })
    }

    fn commit_reserved_locked(&mut self, len: u32, notify: &mut bool) -> Result<(), StreamError> {
        if !self.reserved_active {
            return Err(StreamError::Invalid);
        }
        if len as usize > self.reserved_len {
            return Err(StreamError::NoSpace);
        }

        self.pending_chunk_index = self.reserved_chunk_index;
        self.pending_offset = self.reserved_offset;
        self.pending_len = len as usize;
        self.write_offset = self.reserved_offset + len as usize;
        self.reserved_active = false;
        self.reserved_len = 0;
        self.stats.bytes_written += len as u64;

        self.commit_locked(notify)
    }

    fn add_chunk_locked(&mut self) -> Result<(), StreamError> {
        let chunk_size = self.options.chunk_size;
        let allocated = (self.chunks.len() as u64).saturating_mul(chunk_size as u64);
        if self.options.max_bytes != 0
            && allocated.saturating_add(chunk_size as u64) > self.options.max_bytes
        {
            return Err(StreamError::MaxBytes);
        }

        self.ensure_state_capacity(u32::try_from(self.chunks.len() + 1).unwrap_or(u32::MAX))?;
        let chunk = Chunk::new(chunk_size)?;
        self.chunks.push(chunk);
        self.stats.chunks = u32::try_from(self.chunks.len()).unwrap_or(u32::MAX);
        if let Some(last) = self.chunks.last() {
            self.emit_chunk_added(last);
        }
        Ok(())
    }

    fn ensure_writable_chunk_locked(&mut self) -> Result<(), StreamError> {
        if self.chunks.is_empty() {
            return Err(StreamError::Invalid);
        }

        let total = self.chunks.len();
        let mut attempts = 0usize;
        let mut index = self.current_chunk_index % total;
        while attempts < total {
            if self.is_chunk_free(index) {
                self.current_chunk_index = index;
                self.write_offset = 0;
                self.pending_chunk_index = index;
                self.pending_offset = 0;
                self.pending_len = 0;
                return Ok(());
            }
            index = (index + 1) % total;
            attempts += 1;
        }

        if self.options.growth_policy == GrowthPolicy::Block as u8 {
            return Err(StreamError::NoSpace);
        }

        self.add_chunk_locked()?;
        self.current_chunk_index = self.chunks.len() - 1;
        self.write_offset = 0;
        self.pending_chunk_index = self.current_chunk_index;
        self.pending_offset = 0;
        self.pending_len = 0;
        Ok(())
    }

    fn finish(&self, notify: bool, queued_override: u32) {
        if !notify {
            return;
        }
        let queued = if queued_override != 0 {
            queued_override
        } else {
            u32::try_from(self.span_ring.len()).unwrap_or(u32::MAX)
        };
        if queued > 0 {
            self.emit_data_available(queued);
        }
    }

    fn emit_chunk_added(&self, chunk: &Chunk) {
        if let Some(callback) = self.callback {
            callback(
                self as *const Self as usize,
                EventId::ChunkAdded as u32,
                chunk.ptr() as usize,
                chunk.len() as u64,
            );
        }
    }

    fn emit_data_available(&self, count: u32) {
        if let Some(callback) = self.callback {
            callback(
                self as *const Self as usize,
                EventId::DataAvailable as u32,
                count as usize,
                0,
            );
        }
    }

    fn emit_state_buffer(&self) {
        if let Some(callback) = self.callback {
            callback(
                self as *const Self as usize,
                EventId::StateBuffer as u32,
                self.state_buffer.as_ptr() as usize,
                self.state_buffer.len() as u64,
            );
        }
    }

    fn emit_closed(&self) {
        if let Some(callback) = self.callback {
            callback(self as *const Self as usize, EventId::Closed as u32, 0, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EventId, GrowthPolicy, Options, SpanInfo, Stream, default_options, normalize_options,
    };
    use std::sync::{Mutex, OnceLock};

    static EVENT_LOG: OnceLock<Mutex<Vec<(u32, usize, u64)>>> = OnceLock::new();

    fn event_log() -> &'static Mutex<Vec<(u32, usize, u64)>> {
        EVENT_LOG.get_or_init(|| Mutex::new(Vec::new()))
    }

    extern "C" fn callback(_stream_ptr: usize, event_id: u32, arg0: usize, arg1: u64) {
        event_log().lock().unwrap().push((event_id, arg0, arg1));
    }

    #[test]
    fn normalize_applies_defaults() {
        let options = normalize_options(Options {
            chunk_size: 0,
            initial_chunks: 0,
            max_bytes: 0,
            growth_policy: GrowthPolicy::Grow as u8,
            auto_commit_on_full: 1,
            span_queue_capacity: 0,
        });

        assert_eq!(options.chunk_size, 64 * 1024);
        assert_eq!(options.initial_chunks, 1);
        assert_eq!(options.span_queue_capacity, 4096);
    }

    #[test]
    fn write_commit_and_drain_round_trip() {
        let mut stream = Stream::create(default_options()).unwrap();
        stream.attach().unwrap();
        stream.write(b"hello").unwrap();
        stream.commit().unwrap();

        let mut out = [SpanInfo::default(); 2];
        let count = stream.drain_spans(&mut out);
        assert_eq!(count, 1);
        assert_eq!(out[0].len, 5);
        assert_eq!(stream.get_stats().spans_committed, 1);
    }

    #[test]
    fn reserve_commit_reserved_round_trip() {
        let mut stream = Stream::create(Options {
            chunk_size: 16,
            initial_chunks: 1,
            max_bytes: 0,
            growth_policy: GrowthPolicy::Grow as u8,
            auto_commit_on_full: 0,
            span_queue_capacity: 8,
        })
        .unwrap();

        let reservation = stream.reserve(4).unwrap();
        let slice = unsafe { std::slice::from_raw_parts_mut(reservation.ptr as *mut u8, 4) };
        slice.copy_from_slice(b"rust");
        stream.commit_reserved(4).unwrap();

        let mut out = [SpanInfo::default(); 2];
        assert_eq!(stream.drain_spans(&mut out), 1);
        assert_eq!(out[0].len, 4);
    }

    #[test]
    fn attach_emits_state_chunk_and_data_events() {
        event_log().lock().unwrap().clear();
        let mut stream = Stream::create(default_options()).unwrap();
        stream.set_callback(Some(callback));
        stream.write(b"queued").unwrap();
        stream.commit().unwrap();
        stream.attach().unwrap();

        let captured = event_log().lock().unwrap().clone();
        assert!(
            captured
                .iter()
                .any(|(event, _, _)| *event == EventId::StateBuffer as u32)
        );
        assert!(
            captured
                .iter()
                .any(|(event, _, _)| *event == EventId::ChunkAdded as u32)
        );
        assert!(
            captured
                .iter()
                .any(|(event, _, _)| *event == EventId::DataAvailable as u32)
        );
    }
}
