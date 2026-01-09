
/// IoUring -> the ring itself (shared memory with the kernel)
/// opcode -> what operation to perform (read, write, etc.)
/// types -> wrappers for Linux kernel types (FDs, fixed files, etc)
use io_uring::{IoUring, opcode, types};

/// File -> normal file in the filesystem
use std::fs::File;

/// AsRawFd -> lets us extract the raw Linux file descriptor, the Kernel does not understand `File`.
/// It only understands integers (int fd)
use std::os::unix::io::AsRawFd;

/// This function takes the file path as input and outputs;
/// Ok(n) -> number of bytes read
/// Err(e) -> an OS error
///
/// This function is not asynchronous in Rust terms, it's;
/// - asynchronous in the Kernel
/// - synchronous in the thread it's in
///
/// [TODO]: Remove `submit_and_wait()` and poll manually
#[allow(unused_doc_comments)]
pub fn read_one_file(path: &str) -> std::io::Result<usize> {

    /// Step 1: Create the ring "Create shared memory queues that can hold up to 8(in this case) in-flight requests"
    /// Why 8? No reason, this is arbitrary, it just needs to be >= number of requests you'll submit.
    ///
    /// This call:
    /// - Allocates memory
    /// - Makes a syscall (io_uring_setup)
    /// - Returns a handle to the ring
    ///
    /// IoUring::new() creates a new IoUring instance with default configuration parameters.
    /// The entries set the size of the queue, and its value should be the power of two. The IoUring can be customized using
    /// ::builder()
    let mut ring = IoUring::new(8)?;

    /// Step 2: Open the file
    /// There are two important parts; File::open "normal blocking Rust I/O", as_raw_fd "extract the raw Unix file descriptor (RawFd, which is a type alias for i32)"
    ///
    /// NOTE: The file must stay alive until the I/O finishes (dropping it closes the FD). This is `file` is kept in scope
    let file = File::open(path)?;
    let fd = file.as_raw_fd();

    /// Step 3: Allocate a buffer "where the linux kernel will write data"
    /// Why 4096? This is the typical filesystem block size, it is not required but convenient
    ///
    /// NOTE: The buffer must not move in memory while the kernel is using it. This is why fixed buffers exist
    let mut buffer = vec![0u8; 4096];

    /// Step 4: Prepare the read request (SQE: Submission Queue Entry) "Kernel, please read `len` bytes from file `fd` starting at offset `0`, and write them into this memory address"
    ///
    /// opcode::Read::new(...) constructs a read request, not execution with arguments:
    /// - types::Fd(fd): Read from this file descriptor
    /// - buffer.as_mut_ptr(): Raw pointer (*mut u8), the kernel needs a memory address, not a Rust reference
    /// - buffer.len() as u32: Number of bytes to read
    ///
    /// .offset(0): Tells read where to start from, in this case it is the start of the file.
    /// .build(): Turns the builder into an actual Submission Queue Entry, basically "Freeze the request into a kernel-understandable format"
    /// .user_data(0xdead_beef): This is the tag, kernel will copy this value back into the completion event inorder to;
    ///     - Identify which request completed, especiall when many requests are submitted
    let read_e = opcode::Read::new(
        types::Fd(fd),
        buffer.as_mut_ptr(),
        buffer.len() as u32,
    )
    .offset(0)
    .build()
    .user_data(0xdead_beef);

    /// Step 5: Push request into submission queue
    /// Why unsafe? Writing into shared memory, Rust cannot guarantee the kernel won't misuse it.
    ///
    /// This basically places the read request into the Submission Queue, it does not execute it yet. Nothing happens until it is actually submitted.
    unsafe {
        ring.submission()
            .push(&read_e)
            .expect("Submission queue is full");
    }

    /// Step 6: Submit & wait
    /// This does two things at the same time:
    /// - Tells the kernel "I've added requests. Go process them"
    /// - Blocks until atleast one completion exists
    ///
    /// At this point:
    /// - Kernel reads the file
    /// - DMA / page cache / disk happens
    /// - Kernel writes result into Completion Queue (CQ)
    ring.submit_and_wait(1)?;

    /// Step 7: Read completion (CQE: Completion Queue Entry)
    /// - Pulls one completion event
    /// - This is the kernel saying "That request you submitted? It's done"
    let cqe = ring.completion().next().unwrap();

    /// Step 8: Interpret result
    /// - res >= 0 bytes were read
    /// - res < 0 means -errno
    ///
    /// This is why:
    /// ```rust
    /// if res < 0 {
    ///     return Err(std::os::Error::from_raw_os_error(-res));
    /// }
    /// ```
    let res = cqe.result();

    if res < 0 {
        return Err(std::io::Error::from_raw_os_error(-res));
    }

    /// Step 9: Return bytes read
    /// At this point:
    /// - buffer[..res] contains file data
    /// - Kernel is done touching memory
    Ok(res as usize)
}
