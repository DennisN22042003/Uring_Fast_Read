A high-performance, asynchronous batch file reader built in Rust using `io_uring`. This project demonstrates how to bypass the overhead of traditional syscalls by utilizing *Fixed Buffers* and *Registered Files* to achieve maximum I/O throughput.

# Objective
The goal is to read a large set of files (e.g., thousands of small metadata files or several large log files) into memory as fast as the hardware allows.
By pre-registering resources with the Linux kernel, we minimize the CPU cost of "page pinning" and file descriptor lookups.

# Features
- *Asynchronous Submission:* Batch multiple read requests into a single kernel transition.
- *Registered Files:* Using `IORING_REGISTER_FILES` to pass an array of file descriptors to the kernel once, using indices for subsequent reads.
- *Fixed Buffers:* Using `IORING_REGISTER_BUFFERS` to pre-map memory regions, enabling zero-copy DMA (Direct Memory Access) from the disk controller to userspace.
- *Submission/Completion Loop:* A clean separation between the request producer (SQ) and the result consumer (CQ).

# Architecture

## 1. Setup Phase
- *Initialize Ring:* Create an `io_uring` instance with `IORING_SETUP_SQPOLL` for even lower latency.
- *File Registration:* Open a list of target files, collect their raw FDs, and register them as a fixed set.
- *Buffer Registration:* Allocate a contiguous block of memory (could be a vector of buffers) and register it. Each buffer should be sized to match the filesystem block size (usually 4KB) or the expected read size.

## 2. The Execution Loop
- *SQE Generation:* For each file, create a `ReadFixed` operation.
  - Point the entry to the `Fixed(index)` of the file.
  - Assign it a `buf_index` from your registered buffer pool.
  - Set `user_data` to a unique ID (e.g., an index) so you know which file is finished
- *Submission:* Call `submit_and_wait(batch_size)` to enter the kernel.

## 3. Completion Phase
- *CQE Processing:* Iterate through the completion queue.
- *Error handling:* Check for negative return values (standard Linux error codes).
- *Data Usage:* The data is now available in your pre-allocated buffers. Simply calculate a checksum or count occurrences of a string to verify the read.

# Crates Used
- `io_uring` (low-level API)
- `libc` (for raw file descriptor handling)
