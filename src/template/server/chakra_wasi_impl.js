// WASI constants
const WASI = {
  // Error codes
  ERRNO_SUCCESS: 0,
  ERRNO_BADF: 8,
  ERRNO_INVAL: 28,
  ERRNO_IO: 29,
  ERRNO_NOENT: 44,
  ERRNO_NOSYS: 52,

  // File descriptors
  FD_STDIN: 0,
  FD_STDOUT: 1,
  FD_STDERR: 2,

  // Rights
  RIGHTS_FD_READ: 1n << 0n,
  RIGHTS_FD_WRITE: 1n << 1n,
  RIGHTS_FD_SEEK: 1n << 2n,
  RIGHTS_FD_TELL: 1n << 3n,
  RIGHTS_FD_FDSTAT_SET_FLAGS: 1n << 4n,
  RIGHTS_FD_SYNC: 1n << 5n,
  RIGHTS_FD_ADVISE: 1n << 6n,
  RIGHTS_PATH_CREATE_DIRECTORY: 1n << 7n,
  RIGHTS_PATH_CREATE_FILE: 1n << 8n,
  RIGHTS_PATH_LINK_SOURCE: 1n << 9n,
  RIGHTS_PATH_LINK_TARGET: 1n << 10n,
  RIGHTS_PATH_OPEN: 1n << 11n,
  RIGHTS_FD_READDIR: 1n << 12n,
  RIGHTS_PATH_READLINK: 1n << 13n,
  RIGHTS_PATH_RENAME_SOURCE: 1n << 14n,
  RIGHTS_PATH_RENAME_TARGET: 1n << 15n,
  RIGHTS_PATH_SYMLINK: 1n << 16n,
  RIGHTS_PATH_REMOVE_DIRECTORY: 1n << 17n,
  RIGHTS_PATH_UNLINK_FILE: 1n << 18n,
  RIGHTS_POLL_FD_READWRITE: 1n << 19n,
  RIGHTS_SOCK_SHUTDOWN: 1n << 20n,
};

// WASI file types
const FILETYPE = {
  UNKNOWN: 0,
  BLOCK_DEVICE: 1,
  CHARACTER_DEVICE: 2,
  DIRECTORY: 3,
  REGULAR_FILE: 4,
  SOCKET_DGRAM: 5,
  SOCKET_STREAM: 6,
  SYMBOLIC_LINK: 7,
};

// Virtual filesystem for WASI
class WasiFS {
  constructor() {
    this.files = new Map();
    this.fileDescriptors = new Map();
    this.nextFd = 3; // Start after stdin/stdout/stderr

    // Initialize the root directory
    this.files.set("/", {
      type: FILETYPE.DIRECTORY,
      content: new Map(), // Map of filename -> path
      parent: null,
    });

    // Set up standard file descriptors
    this.fileDescriptors.set(WASI.FD_STDIN, {
      path: "stdin",
      rights: WASI.RIGHTS_FD_READ,
      type: FILETYPE.CHARACTER_DEVICE,
      position: 0,
      content: new Uint8Array(0),
    });

    this.fileDescriptors.set(WASI.FD_STDOUT, {
      path: "stdout",
      rights: WASI.RIGHTS_FD_WRITE,
      type: FILETYPE.CHARACTER_DEVICE,
      position: 0,
      content: null,
    });

    this.fileDescriptors.set(WASI.FD_STDERR, {
      path: "stderr",
      rights: WASI.RIGHTS_FD_WRITE,
      type: FILETYPE.CHARACTER_DEVICE,
      position: 0,
      content: null,
    });
  }

  // Create a directory
  mkdir(path) {
    // Normalize the path
    path = this._normalizePath(path);

    // Check if already exists
    if (this.files.has(path)) {
      return WASI.ERRNO_INVAL;
    }

    // Get parent directory
    const parentPath = this._getParentPath(path);
    if (!this.files.has(parentPath)) {
      return WASI.ERRNO_NOENT;
    }

    const parent = this.files.get(parentPath);
    if (parent.type !== FILETYPE.DIRECTORY) {
      return WASI.ERRNO_INVAL;
    }

    // Create the directory
    const name = this._getBasename(path);
    this.files.set(path, {
      type: FILETYPE.DIRECTORY,
      content: new Map(),
      parent: parentPath,
    });

    // Add to parent directory
    parent.content.set(name, path);

    return WASI.ERRNO_SUCCESS;
  }

  // Create a file
  writeFile(path, data) {
    // Normalize the path
    path = this._normalizePath(path);

    // Get parent directory
    const parentPath = this._getParentPath(path);
    if (!this.files.has(parentPath)) {
      // Try to create parent directories
      if (this.mkdir(parentPath) !== WASI.ERRNO_SUCCESS) {
        return WASI.ERRNO_NOENT;
      }
    }

    const parent = this.files.get(parentPath);
    if (parent.type !== FILETYPE.DIRECTORY) {
      return WASI.ERRNO_INVAL;
    }

    // Convert data to Uint8Array if it's a string
    if (typeof data === "string") {
      const encoder = new TextEncoder();
      data = encoder.encode(data);
    }

    // Create or update the file
    const name = this._getBasename(path);
    const fileExists = this.files.has(path);

    this.files.set(path, {
      type: FILETYPE.REGULAR_FILE,
      content: data,
      parent: parentPath,
    });

    // Add to parent directory if new file
    if (!fileExists) {
      parent.content.set(name, path);
    }

    return WASI.ERRNO_SUCCESS;
  }

  // Read file contents
  readFile(path) {
    // Normalize the path
    path = this._normalizePath(path);

    // Check if file exists
    if (!this.files.has(path)) {
      return null;
    }

    const file = this.files.get(path);
    if (file.type !== FILETYPE.REGULAR_FILE) {
      return null;
    }

    return file.content;
  }

  // List directory contents
  readdir(path) {
    // Normalize the path
    path = this._normalizePath(path);

    // Check if directory exists
    if (!this.files.has(path)) {
      return null;
    }

    const dir = this.files.get(path);
    if (dir.type !== FILETYPE.DIRECTORY) {
      return null;
    }

    return Array.from(dir.content.keys());
  }

  // Open a file and return a file descriptor
  open(path, flags = 0) {
    // Normalize the path
    path = this._normalizePath(path);

    // Check if file exists
    if (!this.files.has(path)) {
      return { fd: -1, errno: WASI.ERRNO_NOENT };
    }

    const file = this.files.get(path);

    // Determine rights based on file type
    let rights = 0n;
    if (file.type === FILETYPE.REGULAR_FILE) {
      rights =
        WASI.RIGHTS_FD_READ |
        WASI.RIGHTS_FD_WRITE |
        WASI.RIGHTS_FD_SEEK |
        WASI.RIGHTS_FD_TELL |
        WASI.RIGHTS_FD_SYNC;
    } else if (file.type === FILETYPE.DIRECTORY) {
      rights = WASI.RIGHTS_FD_READDIR | WASI.RIGHTS_PATH_OPEN;
    }

    // Create file descriptor
    const fd = this.nextFd++;
    this.fileDescriptors.set(fd, {
      path: path,
      rights: rights,
      type: file.type,
      position: 0,
      content: file.type === FILETYPE.REGULAR_FILE ? file.content : null,
    });

    return { fd, errno: WASI.ERRNO_SUCCESS };
  }

  // Close a file descriptor
  close(fd) {
    if (!this.fileDescriptors.has(fd)) {
      return WASI.ERRNO_BADF;
    }

    // Don't close standard file descriptors
    if (fd <= 2) {
      return WASI.ERRNO_SUCCESS;
    }

    this.fileDescriptors.delete(fd);
    return WASI.ERRNO_SUCCESS;
  }

  // Read from file descriptor
  read(fd, buffer, offset, length) {
    if (!this.fileDescriptors.has(fd)) {
      return { bytesRead: 0, errno: WASI.ERRNO_BADF };
    }

    const fileDesc = this.fileDescriptors.get(fd);

    // Check if file is readable
    if (!(fileDesc.rights & WASI.RIGHTS_FD_READ)) {
      return { bytesRead: 0, errno: WASI.ERRNO_BADF };
    }

    // Handle special case for stdin
    if (fd === WASI.FD_STDIN) {
      // For now, return 0 bytes (would need proper stdin handling in a real implementation)
      return { bytesRead: 0, errno: WASI.ERRNO_SUCCESS };
    }

    // Regular file handling
    if (fileDesc.type === FILETYPE.REGULAR_FILE && fileDesc.content) {
      const position = fileDesc.position;
      const content = fileDesc.content;

      // Check if we've reached the end of the file
      if (position >= content.byteLength) {
        return { bytesRead: 0, errno: WASI.ERRNO_SUCCESS };
      }

      // Calculate how many bytes to read
      const bytesToRead = Math.min(length, content.byteLength - position);

      // Copy data to the target buffer
      buffer.set(content.subarray(position, position + bytesToRead), offset);

      // Update file position
      fileDesc.position += bytesToRead;

      return { bytesRead: bytesToRead, errno: WASI.ERRNO_SUCCESS };
    }

    return { bytesRead: 0, errno: WASI.ERRNO_INVAL };
  }

  // Write to file descriptor
  write(fd, buffer, offset, length) {
    if (!this.fileDescriptors.has(fd)) {
      return { bytesWritten: 0, errno: WASI.ERRNO_BADF };
    }

    const fileDesc = this.fileDescriptors.get(fd);

    // Check if file is writable
    if (!(fileDesc.rights & WASI.RIGHTS_FD_WRITE)) {
      return { bytesWritten: 0, errno: WASI.ERRNO_BADF };
    }

    // Handle special cases for stdout and stderr
    if (fd === WASI.FD_STDOUT || fd === WASI.FD_STDERR) {
      // For stdout/stderr, we extract the text and pass to console
      const data = buffer.subarray(offset, offset + length);
      const text = new TextDecoder().decode(data);

      if (fd === WASI.FD_STDOUT) {
        console.log(text);
      } else {
        console.error(text);
      }

      return { bytesWritten: length, errno: WASI.ERRNO_SUCCESS };
    }

    // Regular file handling
    if (fileDesc.type === FILETYPE.REGULAR_FILE) {
      const position = fileDesc.position;

      // If the file content array is too small, resize it
      if (fileDesc.content.byteLength < position + length) {
        const newContent = new Uint8Array(position + length);
        newContent.set(fileDesc.content);
        fileDesc.content = newContent;

        // Also update the file in the filesystem
        if (fileDesc.path && this.files.has(fileDesc.path)) {
          this.files.get(fileDesc.path).content = fileDesc.content;
        }
      }

      // Copy the data to the file
      fileDesc.content.set(buffer.subarray(offset, offset + length), position);

      // Update file position
      fileDesc.position += length;

      return { bytesWritten: length, errno: WASI.ERRNO_SUCCESS };
    }

    return { bytesWritten: 0, errno: WASI.ERRNO_INVAL };
  }

  // Seek in a file
  seek(fd, offset, whence) {
    if (!this.fileDescriptors.has(fd)) {
      return { position: 0, errno: WASI.ERRNO_BADF };
    }

    const fileDesc = this.fileDescriptors.get(fd);

    // Check if file supports seeking
    if (!(fileDesc.rights & WASI.RIGHTS_FD_SEEK)) {
      return { position: 0, errno: WASI.ERRNO_BADF };
    }

    // Regular file handling
    if (fileDesc.type === FILETYPE.REGULAR_FILE) {
      let newPosition;

      // SEEK_SET
      if (whence === 0) {
        newPosition = offset;
      }
      // SEEK_CUR
      else if (whence === 1) {
        newPosition = fileDesc.position + offset;
      }
      // SEEK_END
      else if (whence === 2) {
        newPosition = fileDesc.content.byteLength + offset;
      } else {
        return { position: 0, errno: WASI.ERRNO_INVAL };
      }

      // Check if position is valid
      if (newPosition < 0) {
        return { position: 0, errno: WASI.ERRNO_INVAL };
      }

      // Update file position
      fileDesc.position = newPosition;

      return { position: newPosition, errno: WASI.ERRNO_SUCCESS };
    }

    return { position: 0, errno: WASI.ERRNO_INVAL };
  }

  // Get file descriptor stats
  fdstat(fd) {
    if (!this.fileDescriptors.has(fd)) {
      return { errno: WASI.ERRNO_BADF };
    }

    const fileDesc = this.fileDescriptors.get(fd);

    return {
      filetype: fileDesc.type,
      rights_base: fileDesc.rights,
      rights_inheriting: 0n,
      flags: 0,
      errno: WASI.ERRNO_SUCCESS,
    };
  }

  // Helper: Normalize path
  _normalizePath(path) {
    if (!path.startsWith("/")) {
      path = "/" + path;
    }

    // Remove trailing slash except for root
    if (path.length > 1 && path.endsWith("/")) {
      path = path.slice(0, -1);
    }

    return path;
  }

  // Helper: Get parent directory path
  _getParentPath(path) {
    path = this._normalizePath(path);

    if (path === "/") {
      return "/";
    }

    const lastSlash = path.lastIndexOf("/");
    if (lastSlash === 0) {
      return "/";
    }

    return path.substring(0, lastSlash);
  }

  // Helper: Get basename of path
  _getBasename(path) {
    path = this._normalizePath(path);

    if (path === "/") {
      return "";
    }

    const lastSlash = path.lastIndexOf("/");
    return path.substring(lastSlash + 1);
  }
}

// WASI Implementation
class WASIImplementation {
  constructor(options = {}) {
    this.options = {
      args: options.args || [],
      env: options.env || {},
      preopens: options.preopens || { "/": "/" },
      stdout: options.stdout || ((text) => console.log(text)),
      stderr: options.stderr || ((text) => console.error(text)),
      stdin: options.stdin || (() => null),
      ...options,
    };

    this.memory = null;
    this.view = null;
    this.instance = null;

    // Initialize virtual filesystem
    this.fs = new WasiFS();

    // Set up any preopen directories
    for (const [guest, host] of Object.entries(this.options.preopens)) {
      this.fs.mkdir(guest);
    }
  }

  // Initialize the WASI instance with WebAssembly instance
  initialize(instance) {
    this.instance = instance;
    this.memory = instance.exports.memory;
    this.refreshMemory();
  }

  // Refresh memory view after potential memory growth
  refreshMemory() {
    this.view = new DataView(this.memory.buffer);
  }

  // Get import object for WebAssembly instantiation
  getImportObject() {
    return {
      wasi_snapshot_preview1: {
        args_get: (argv, argv_buf) => this.args_get(argv, argv_buf),
        args_sizes_get: (argc, argv_buf_size) =>
          this.args_sizes_get(argc, argv_buf_size),
        environ_get: (environ, environ_buf) =>
          this.environ_get(environ, environ_buf),
        environ_sizes_get: (environ_count, environ_buf_size) =>
          this.environ_sizes_get(environ_count, environ_buf_size),
        clock_res_get: (clock_id, resolution) =>
          this.clock_res_get(clock_id, resolution),
        clock_time_get: (clock_id, precision, time) =>
          this.clock_time_get(clock_id, precision, time),
        fd_advise: (fd, offset, len, advice) =>
          this.fd_advise(fd, offset, len, advice),
        fd_allocate: (fd, offset, len) => this.fd_allocate(fd, offset, len),
        fd_close: (fd) => this.fd_close(fd),
        fd_datasync: (fd) => this.fd_datasync(fd),
        fd_fdstat_get: (fd, stat) => this.fd_fdstat_get(fd, stat),
        fd_fdstat_set_flags: (fd, flags) => this.fd_fdstat_set_flags(fd, flags),
        fd_fdstat_set_rights: (fd, fs_rights_base, fs_rights_inheriting) =>
          this.fd_fdstat_set_rights(fd, fs_rights_base, fs_rights_inheriting),
        fd_filestat_get: (fd, buf) => this.fd_filestat_get(fd, buf),
        fd_filestat_set_size: (fd, size) => this.fd_filestat_set_size(fd, size),
        fd_filestat_set_times: (fd, atim, mtim, fst_flags) =>
          this.fd_filestat_set_times(fd, atim, mtim, fst_flags),
        fd_pread: (fd, iovs, iovs_len, offset, nread) =>
          this.fd_pread(fd, iovs, iovs_len, offset, nread),
        fd_prestat_get: (fd, buf) => this.fd_prestat_get(fd, buf),
        fd_prestat_dir_name: (fd, path, path_len) =>
          this.fd_prestat_dir_name(fd, path, path_len),
        fd_pwrite: (fd, iovs, iovs_len, offset, nwritten) =>
          this.fd_pwrite(fd, iovs, iovs_len, offset, nwritten),
        fd_read: (fd, iovs, iovs_len, nread) =>
          this.fd_read(fd, iovs, iovs_len, nread),
        fd_readdir: (fd, buf, buf_len, cookie, bufused) =>
          this.fd_readdir(fd, buf, buf_len, cookie, bufused),
        fd_renumber: (fd, to) => this.fd_renumber(fd, to),
        fd_seek: (fd, offset, whence, newoffset) =>
          this.fd_seek(fd, offset, whence, newoffset),
        fd_sync: (fd) => this.fd_sync(fd),
        fd_tell: (fd, offset) => this.fd_tell(fd, offset),
        fd_write: (fd, iovs, iovs_len, nwritten) =>
          this.fd_write(fd, iovs, iovs_len, nwritten),
        path_create_directory: (fd, path, path_len) =>
          this.path_create_directory(fd, path, path_len),
        path_filestat_get: (fd, flags, path, path_len, buf) =>
          this.path_filestat_get(fd, flags, path, path_len, buf),
        path_filestat_set_times: (
          fd,
          flags,
          path,
          path_len,
          atim,
          mtim,
          fst_flags
        ) =>
          this.path_filestat_set_times(
            fd,
            flags,
            path,
            path_len,
            atim,
            mtim,
            fst_flags
          ),
        path_link: (
          old_fd,
          old_flags,
          old_path,
          old_path_len,
          new_fd,
          new_path,
          new_path_len
        ) =>
          this.path_link(
            old_fd,
            old_flags,
            old_path,
            old_path_len,
            new_fd,
            new_path,
            new_path_len
          ),
        path_open: (
          fd,
          dirflags,
          path,
          path_len,
          oflags,
          fs_rights_base,
          fs_rights_inheriting,
          fdflags,
          opened_fd
        ) =>
          this.path_open(
            fd,
            dirflags,
            path,
            path_len,
            oflags,
            fs_rights_base,
            fs_rights_inheriting,
            fdflags,
            opened_fd
          ),
        path_readlink: (fd, path, path_len, buf, buf_len, bufused) =>
          this.path_readlink(fd, path, path_len, buf, buf_len, bufused),
        path_remove_directory: (fd, path, path_len) =>
          this.path_remove_directory(fd, path, path_len),
        path_rename: (
          fd,
          old_path,
          old_path_len,
          new_fd,
          new_path,
          new_path_len
        ) =>
          this.path_rename(
            fd,
            old_path,
            old_path_len,
            new_fd,
            new_path,
            new_path_len
          ),
        path_symlink: (old_path, old_path_len, fd, new_path, new_path_len) =>
          this.path_symlink(old_path, old_path_len, fd, new_path, new_path_len),
        path_unlink_file: (fd, path, path_len) =>
          this.path_unlink_file(fd, path, path_len),
        poll_oneoff: (in_ptr, out_ptr, nsubscriptions, nevents) =>
          this.poll_oneoff(in_ptr, out_ptr, nsubscriptions, nevents),
        proc_exit: (rval) => this.proc_exit(rval),
        proc_raise: (sig) => this.proc_raise(sig),
        random_get: (buf, buf_len) => this.random_get(buf, buf_len),
        sched_yield: () => this.sched_yield(),
        // Socket functions
        sock_accept: (fd, flags, result_fd) => {
          // Return ENOSYS (function not implemented)
          return WASI.ERRNO_NOSYS;
        },

        sock_recv: (fd, ri_data, ri_flags, ro_datalen, ro_flags) => {
          // Return ENOSYS (function not implemented)
          return WASI.ERRNO_NOSYS;
        },

        sock_send: (fd, si_data, si_flags, so_datalen) => {
          // Return ENOSYS (function not implemented)
          return WASI.ERRNO_NOSYS;
        },

        sock_shutdown: (fd, how) => {
          // Return ENOSYS (function not implemented)
          return WASI.ERRNO_NOSYS;
        },

        // These are additional socket-related functions that might be needed
        sock_open: (af, socktype, protocol, fd) => {
          // Return ENOSYS (function not implemented)
          return WASI.ERRNO_NOSYS;
        },

        sock_bind: (fd, addr, port) => {
          // Return ENOSYS (function not implemented)
          return WASI.ERRNO_NOSYS;
        },

        sock_connect: (fd, addr, port) => {
          // Return ENOSYS (function not implemented)
          return WASI.ERRNO_NOSYS;
        },

        sock_listen: (fd, backlog) => {
          // Return ENOSYS (function not implemented)
          return WASI.ERRNO_NOSYS;
        },

        sock_getsockopt: (fd, level, optname, optval, optlen) => {
          // Return ENOSYS (function not implemented)
          return WASI.ERRNO_NOSYS;
        },

        sock_setsockopt: (fd, level, optname, optval, optlen) => {
          // Return ENOSYS (function not implemented)
          return WASI.ERRNO_NOSYS;
        },

        sock_getlocaladdr: (fd, addr, addrlen) => {
          // Return ENOSYS (function not implemented)
          return WASI.ERRNO_NOSYS;
        },

        sock_getpeeraddr: (fd, addr, addrlen) => {
          // Return ENOSYS (function not implemented)
          return WASI.ERRNO_NOSYS;
        },

        sock_getaddrinfo: (node, service, hints, res, maxres, reslen) => {
          // Return ENOSYS (function not implemented)
          return WASI.ERRNO_NOSYS;
        },
      },
    };
  }

  // Helper to read a string from memory
  readString(ptr, len) {
    this.refreshMemory();
    const buffer = new Uint8Array(this.memory.buffer, ptr, len);
    return new TextDecoder().decode(buffer);
  }

  // Helper to write a string to memory
  writeString(ptr, str) {
    this.refreshMemory();
    const buffer = new Uint8Array(this.memory.buffer, ptr, str.length);
    const encoder = new TextEncoder();
    buffer.set(encoder.encode(str));
  }

  // WASI implementation: args_get
  args_get(argv, argv_buf) {
    this.refreshMemory();

    const args = this.options.args;
    let bufferOffset = argv_buf;

    for (let i = 0; i < args.length; i++) {
      this.view.setUint32(argv + i * 4, bufferOffset, true); // Set pointer

      const encoder = new TextEncoder();
      const data = encoder.encode(args[i]);
      const buf = new Uint8Array(
        this.memory.buffer,
        bufferOffset,
        data.length + 1
      );
      buf.set(data);
      buf[data.length] = 0; // null terminator

      bufferOffset += data.length + 1;
    }

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: args_sizes_get
  args_sizes_get(argc, argv_buf_size) {
    this.refreshMemory();

    const args = this.options.args;
    let bufSize = 0;

    for (let i = 0; i < args.length; i++) {
      bufSize += args[i].length + 1; // +1 for null terminator
    }

    this.view.setUint32(argc, args.length, true);
    this.view.setUint32(argv_buf_size, bufSize, true);

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: environ_get
  environ_get(environ, environ_buf) {
    this.refreshMemory();

    const env = this.options.env;
    const envEntries = Object.entries(env);
    let bufferOffset = environ_buf;

    for (let i = 0; i < envEntries.length; i++) {
      const [key, value] = envEntries[i];
      const envVar = `${key}=${value}`;

      this.view.setUint32(environ + i * 4, bufferOffset, true); // Set pointer

      const encoder = new TextEncoder();
      const data = encoder.encode(envVar);
      const buf = new Uint8Array(
        this.memory.buffer,
        bufferOffset,
        data.length + 1
      );
      buf.set(data);
      buf[data.length] = 0; // null terminator

      bufferOffset += data.length + 1;
    }

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: environ_sizes_get
  environ_sizes_get(environ_count, environ_buf_size) {
    this.refreshMemory();

    const env = this.options.env;
    const envEntries = Object.entries(env);
    let bufSize = 0;

    for (const [key, value] of envEntries) {
      bufSize += key.length + value.length + 2; // +2 for '=' and null terminator
    }

    this.view.setUint32(environ_count, envEntries.length, true);
    this.view.setUint32(environ_buf_size, bufSize, true);

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: clock_res_get
  clock_res_get(clock_id, resolution) {
    this.refreshMemory();

    // Default to 1000ns (1μs) resolution for all clocks
    this.view.setBigUint64(resolution, BigInt(1000), true);

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: clock_time_get
  clock_time_get(clock_id, precision, time) {
    this.refreshMemory();

    // All clocks just return the current time in nanoseconds
    const now = BigInt(Date.now()) * BigInt(1000000); // milliseconds to nanoseconds
    this.view.setBigUint64(time, now, true);

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: fd_advise
  fd_advise(fd, offset, len, advice) {
    // Not implemented, but return success
    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: fd_allocate
  fd_allocate(fd, offset, len) {
    // Not implemented for browser
    return WASI.ERRNO_NOSYS;
  }

  // WASI implementation: fd_close
  fd_close(fd) {
    return this.fs.close(fd);
  }

  // WASI implementation: fd_datasync
  fd_datasync(fd) {
    // No actual sync needed in browser environment
    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: fd_fdstat_get
  fd_fdstat_get(fd, stat) {
    this.refreshMemory();

    const fdstat = this.fs.fdstat(fd);
    if (fdstat.errno !== WASI.ERRNO_SUCCESS) {
      return fdstat.errno;
    }

    // Fill the stat structure
    this.view.setUint8(stat, fdstat.filetype); // filetype
    this.view.setUint8(stat + 1, 0); // padding
    this.view.setUint16(stat + 2, fdstat.flags, true); // flags
    this.view.setBigUint64(stat + 8, fdstat.rights_base, true); // rights_base
    this.view.setBigUint64(stat + 16, fdstat.rights_inheriting, true); // rights_inheriting

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: fd_fdstat_set_flags
  fd_fdstat_set_flags(fd, flags) {
    // Not fully implemented in browser environment
    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: fd_fdstat_set_rights
  fd_fdstat_set_rights(fd, fs_rights_base, fs_rights_inheriting) {
    // Not fully implemented in browser environment
    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: fd_filestat_get
  fd_filestat_get(fd, buf) {
    this.refreshMemory();

    if (!this.fs.fileDescriptors.has(fd)) {
      return WASI.ERRNO_BADF;
    }

    const fileDesc = this.fs.fileDescriptors.get(fd);

    // Fill the stat structure with zeros first
    for (let i = 0; i < 64; i++) {
      this.view.setUint8(buf + i, 0);
    }

    // Set file type
    this.view.setUint8(buf + 16, fileDesc.type);

    // Set file size for regular files
    if (fileDesc.type === FILETYPE.REGULAR_FILE && fileDesc.content) {
      this.view.setBigUint64(
        buf + 24,
        BigInt(fileDesc.content.byteLength),
        true
      );
    }

    // Set timestamps (current time for all)
    const now = BigInt(Date.now()) * BigInt(1000000); // milliseconds to nanoseconds
    this.view.setBigUint64(buf + 32, now, true); // atime
    this.view.setBigUint64(buf + 40, now, true); // mtime
    this.view.setBigUint64(buf + 48, now, true); // ctime

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: fd_filestat_set_size
  fd_filestat_set_size(fd, size) {
    // Not fully implemented in browser environment
    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: fd_filestat_set_times
  fd_filestat_set_times(fd, atim, mtim, fst_flags) {
    // Not implemented in browser environment
    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: fd_pread
  fd_pread(fd, iovs, iovs_len, offset, nread) {
    this.refreshMemory();

    if (!this.fs.fileDescriptors.has(fd)) {
      return WASI.ERRNO_BADF;
    }

    const fileDesc = this.fs.fileDescriptors.get(fd);

    // Check if file is readable
    if (!(fileDesc.rights & WASI.RIGHTS_FD_READ)) {
      return WASI.ERRNO_BADF;
    }

    // Only supported for regular files
    if (fileDesc.type !== FILETYPE.REGULAR_FILE || !fileDesc.content) {
      return WASI.ERRNO_INVAL;
    }

    // Save the current position
    const originalPosition = fileDesc.position;

    // Set position to the requested offset
    fileDesc.position = Number(offset);

    // Perform the read operation
    const result = this.fd_read(fd, iovs, iovs_len, nread);

    // Restore the original position
    fileDesc.position = originalPosition;

    return result;
  }

  // WASI implementation: fd_prestat_get
  fd_prestat_get(fd, buf) {
    this.refreshMemory();

    // We only support prestat for pre-opened directories
    const preopens = Object.keys(this.options.preopens);

    if (fd >= 3 && fd < 3 + preopens.length) {
      const preopen = preopens[fd - 3];

      // Set prestat type to directory
      this.view.setUint8(buf, 0); // pr_type = 0 (directory)

      // Set name length (excluding null terminator)
      this.view.setUint32(buf + 4, preopen.length, true);

      return WASI.ERRNO_SUCCESS;
    }

    return WASI.ERRNO_BADF;
  }

  // WASI implementation: fd_prestat_dir_name
  fd_prestat_dir_name(fd, path, path_len) {
    this.refreshMemory();

    // We only support prestat for pre-opened directories
    const preopens = Object.keys(this.options.preopens);

    if (fd >= 3 && fd < 3 + preopens.length) {
      const preopen = preopens[fd - 3];

      if (path_len < preopen.length) {
        return WASI.ERRNO_INVAL;
      }

      // Write the directory name to the path buffer
      const encoder = new TextEncoder();
      const encodedPath = encoder.encode(preopen);
      const pathBuffer = new Uint8Array(this.memory.buffer, path, path_len);
      pathBuffer.set(encodedPath);

      return WASI.ERRNO_SUCCESS;
    }

    return WASI.ERRNO_BADF;
  }

  // WASI implementation: fd_pwrite
  fd_pwrite(fd, iovs, iovs_len, offset, nwritten) {
    this.refreshMemory();

    if (!this.fs.fileDescriptors.has(fd)) {
      return WASI.ERRNO_BADF;
    }

    const fileDesc = this.fs.fileDescriptors.get(fd);

    // Check if file is writable
    if (!(fileDesc.rights & WASI.RIGHTS_FD_WRITE)) {
      return WASI.ERRNO_BADF;
    }

    // Only supported for regular files
    if (fileDesc.type !== FILETYPE.REGULAR_FILE) {
      return WASI.ERRNO_INVAL;
    }

    // Save the current position
    const originalPosition = fileDesc.position;

    // Set position to the requested offset
    fileDesc.position = Number(offset);

    // Perform the write operation
    const result = this.fd_write(fd, iovs, iovs_len, nwritten);

    // Restore the original position
    fileDesc.position = originalPosition;

    return result;
  }

  // WASI implementation: fd_read
  fd_read(fd, iovs, iovs_len, nread) {
    this.refreshMemory();

    let totalBytesRead = 0;

    for (let i = 0; i < iovs_len; i++) {
      const iovPtr = iovs + i * 8; // Each iov is 8 bytes (2 u32s)
      const bufPtr = this.view.getUint32(iovPtr, true);
      const bufLen = this.view.getUint32(iovPtr + 4, true);

      if (bufLen === 0) continue;

      const buffer = new Uint8Array(this.memory.buffer, bufPtr, bufLen);

      const { bytesRead, errno } = this.fs.read(fd, buffer, 0, bufLen);

      if (errno !== WASI.ERRNO_SUCCESS) {
        return errno;
      }

      totalBytesRead += bytesRead;

      // If we read fewer bytes than requested, stop reading
      if (bytesRead < bufLen) {
        break;
      }
    }

    // Write the total bytes read
    this.view.setUint32(nread, totalBytesRead, true);

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: fd_readdir
  fd_readdir(fd, buf, buf_len, cookie, bufused) {
    this.refreshMemory();

    if (!this.fs.fileDescriptors.has(fd)) {
      return WASI.ERRNO_BADF;
    }

    const fileDesc = this.fs.fileDescriptors.get(fd);

    // Check if directory
    if (fileDesc.type !== FILETYPE.DIRECTORY) {
      return WASI.ERRNO_NOTDIR;
    }

    // Get directory path
    const dirPath = fileDesc.path;
    if (!dirPath || !this.fs.files.has(dirPath)) {
      return WASI.ERRNO_INVAL;
    }

    const dir = this.fs.files.get(dirPath);
    const entries = Array.from(dir.content.entries());

    let bytesUsed = 0;
    let cookieIndex = Number(cookie);

    // Add special entries for . and ..
    if (cookieIndex === 0) {
      // Current directory (.)
      const entry = {
        name: ".",
        type: FILETYPE.DIRECTORY,
      };

      const bytesWritten = this._writeDirectoryEntry(
        buf,
        bytesUsed,
        buf_len,
        0,
        entry
      );
      if (bytesWritten < 0) {
        // Not enough space
        this.view.setUint32(bufused, bytesUsed, true);
        return WASI.ERRNO_SUCCESS;
      }

      bytesUsed += bytesWritten;
      cookieIndex++;
    }

    if (cookieIndex === 1) {
      // Parent directory (..)
      const entry = {
        name: "..",
        type: FILETYPE.DIRECTORY,
      };

      const bytesWritten = this._writeDirectoryEntry(
        buf,
        bytesUsed,
        buf_len,
        1,
        entry
      );
      if (bytesWritten < 0) {
        // Not enough space
        this.view.setUint32(bufused, bytesUsed, true);
        return WASI.ERRNO_SUCCESS;
      }

      bytesUsed += bytesWritten;
      cookieIndex++;
    }

    // Process regular entries
    for (let i = cookieIndex - 2; i < entries.length; i++) {
      const [name, path] = entries[i];
      const file = this.fs.files.get(path);

      const entry = {
        name,
        type: file.type,
      };

      const entryCookie = i + 2; // +2 for . and .. entries
      const bytesWritten = this._writeDirectoryEntry(
        buf,
        bytesUsed,
        buf_len,
        entryCookie,
        entry
      );
      if (bytesWritten < 0) {
        // Not enough space
        break;
      }

      bytesUsed += bytesWritten;
    }

    this.view.setUint32(bufused, bytesUsed, true);
    return WASI.ERRNO_SUCCESS;
  }

  // Helper for fd_readdir to write a directory entry
  _writeDirectoryEntry(buf, offset, buf_len, d_next, entry) {
    if (offset >= buf_len) {
      return -1; // Not enough space
    }

    const nameBytes = new TextEncoder().encode(entry.name);
    const entrySize = 24 + nameBytes.length;

    if (offset + entrySize > buf_len) {
      return -1; // Not enough space
    }

    // Write directory entry
    this.view.setBigUint64(buf + offset, BigInt(d_next), true); // d_next
    this.view.setBigUint64(buf + offset + 8, BigInt(entry.name.length), true); // d_namlen
    this.view.setUint8(buf + offset + 16, entry.type); // d_type

    // Write name
    const nameBuffer = new Uint8Array(
      this.memory.buffer,
      buf + offset + 24,
      nameBytes.length
    );
    nameBuffer.set(nameBytes);

    return entrySize;
  }

  // WASI implementation: fd_renumber
  fd_renumber(fd, to) {
    if (!this.fs.fileDescriptors.has(fd)) {
      return WASI.ERRNO_BADF;
    }

    if (!this.fs.fileDescriptors.has(to)) {
      return WASI.ERRNO_BADF;
    }

    // Copy file descriptor
    this.fs.fileDescriptors.set(to, this.fs.fileDescriptors.get(fd));

    // Close original fd
    this.fs.fileDescriptors.delete(fd);

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: fd_seek
  fd_seek(fd, offset, whence, newoffset) {
    this.refreshMemory();

    const { position, errno } = this.fs.seek(fd, Number(offset), whence);

    if (errno !== WASI.ERRNO_SUCCESS) {
      return errno;
    }

    this.view.setBigUint64(newoffset, BigInt(position), true);

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: fd_sync
  fd_sync(fd) {
    // No actual sync needed in browser environment
    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: fd_tell
  fd_tell(fd, offset) {
    this.refreshMemory();

    if (!this.fs.fileDescriptors.has(fd)) {
      return WASI.ERRNO_BADF;
    }

    const fileDesc = this.fs.fileDescriptors.get(fd);
    this.view.setBigUint64(offset, BigInt(fileDesc.position), true);

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: fd_write
  fd_write(fd, iovs, iovs_len, nwritten) {
    this.refreshMemory();

    let totalBytesWritten = 0;

    // Handle stdout and stderr separately for direct console output
    if (fd === WASI.FD_STDOUT || fd === WASI.FD_STDERR) {
      let text = "";

      for (let i = 0; i < iovs_len; i++) {
        const iovPtr = iovs + i * 8; // Each iov is 8 bytes (2 u32s)
        const bufPtr = this.view.getUint32(iovPtr, true);
        const bufLen = this.view.getUint32(iovPtr + 4, true);

        if (bufLen === 0) continue;

        const buffer = new Uint8Array(this.memory.buffer, bufPtr, bufLen);
        const chunk = new TextDecoder().decode(buffer);
        text += chunk;
        totalBytesWritten += bufLen;
      }

      // Output to the appropriate console function
      if (fd === WASI.FD_STDOUT) {
        this.options.stdout(text);
      } else {
        this.options.stderr(text);
      }

      this.view.setUint32(nwritten, totalBytesWritten, true);
      return WASI.ERRNO_SUCCESS;
    }

    // Process each I/O vector
    for (let i = 0; i < iovs_len; i++) {
      const iovPtr = iovs + i * 8; // Each iov is 8 bytes (2 u32s)
      const bufPtr = this.view.getUint32(iovPtr, true);
      const bufLen = this.view.getUint32(iovPtr + 4, true);

      if (bufLen === 0) continue;

      const buffer = new Uint8Array(this.memory.buffer, bufPtr, bufLen);

      const { bytesWritten, errno } = this.fs.write(fd, buffer, 0, bufLen);

      if (errno !== WASI.ERRNO_SUCCESS) {
        return errno;
      }

      totalBytesWritten += bytesWritten;
    }

    // Write the total bytes written
    this.view.setUint32(nwritten, totalBytesWritten, true);

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: path_create_directory
  path_create_directory(fd, path_ptr, path_len) {
    this.refreshMemory();

    if (!this.fs.fileDescriptors.has(fd)) {
      return WASI.ERRNO_BADF;
    }

    const fileDesc = this.fs.fileDescriptors.get(fd);

    // Check if directory
    if (fileDesc.type !== FILETYPE.DIRECTORY) {
      return WASI.ERRNO_NOTDIR;
    }

    // Get the path string
    const pathStr = this.readString(path_ptr, path_len);

    // Resolve the full path
    const basePath = fileDesc.path || "/";
    const fullPath = this._resolvePath(basePath, pathStr);

    // Create the directory
    return this.fs.mkdir(fullPath);
  }

  // WASI implementation: path_filestat_get
  path_filestat_get(fd, flags, path_ptr, path_len, buf) {
    this.refreshMemory();

    if (!this.fs.fileDescriptors.has(fd)) {
      return WASI.ERRNO_BADF;
    }

    const fileDesc = this.fs.fileDescriptors.get(fd);

    // Check if directory
    if (fileDesc.type !== FILETYPE.DIRECTORY) {
      return WASI.ERRNO_NOTDIR;
    }

    // Get the path string
    const pathStr = this.readString(path_ptr, path_len);

    // Resolve the full path
    const basePath = fileDesc.path || "/";
    const fullPath = this._resolvePath(basePath, pathStr);

    // Check if file exists
    if (!this.fs.files.has(fullPath)) {
      return WASI.ERRNO_NOENT;
    }

    const file = this.fs.files.get(fullPath);

    // Fill the stat structure with zeros first
    for (let i = 0; i < 64; i++) {
      this.view.setUint8(buf + i, 0);
    }

    // Set file type
    this.view.setUint8(buf + 16, file.type);

    // Set file size for regular files
    if (file.type === FILETYPE.REGULAR_FILE && file.content) {
      this.view.setBigUint64(buf + 24, BigInt(file.content.byteLength), true);
    }

    // Set timestamps (current time for all)
    const now = BigInt(Date.now()) * BigInt(1000000); // milliseconds to nanoseconds
    this.view.setBigUint64(buf + 32, now, true); // atime
    this.view.setBigUint64(buf + 40, now, true); // mtime
    this.view.setBigUint64(buf + 48, now, true); // ctime

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: path_filestat_set_times
  path_filestat_set_times(
    fd,
    flags,
    path_ptr,
    path_len,
    atim,
    mtim,
    fst_flags
  ) {
    // Not implemented in browser environment
    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: path_link
  path_link(
    old_fd,
    old_flags,
    old_path,
    old_path_len,
    new_fd,
    new_path,
    new_path_len
  ) {
    // Not implemented in browser environment
    return WASI.ERRNO_NOSYS;
  }

  // WASI implementation: path_open
  path_open(
    fd,
    dirflags,
    path_ptr,
    path_len,
    oflags,
    fs_rights_base,
    fs_rights_inheriting,
    fdflags,
    opened_fd
  ) {
    this.refreshMemory();

    if (!this.fs.fileDescriptors.has(fd)) {
      return WASI.ERRNO_BADF;
    }

    const fileDesc = this.fs.fileDescriptors.get(fd);

    // Check if directory
    if (fileDesc.type !== FILETYPE.DIRECTORY) {
      return WASI.ERRNO_NOTDIR;
    }

    // Get the path string
    const pathStr = this.readString(path_ptr, path_len);

    // Resolve the full path
    const basePath = fileDesc.path || "/";
    const fullPath = this._resolvePath(basePath, pathStr);

    // Check if the path exists
    const pathExists = this.fs.files.has(fullPath);

    // Handle create flag
    if (!pathExists && oflags & 1) {
      // Create an empty file
      this.fs.writeFile(fullPath, new Uint8Array(0));
    }

    // Open the file
    const { fd: newFd, errno } = this.fs.open(fullPath);

    if (errno !== WASI.ERRNO_SUCCESS) {
      return errno;
    }

    // Write the file descriptor
    this.view.setUint32(opened_fd, newFd, true);

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: path_readlink
  path_readlink(fd, path, path_len, buf, buf_len, bufused) {
    // Not implemented in browser environment
    return WASI.ERRNO_NOSYS;
  }

  // WASI implementation: path_remove_directory
  path_remove_directory(fd, path, path_len) {
    // Not fully implemented in browser environment
    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: path_rename
  path_rename(fd, old_path, old_path_len, new_fd, new_path, new_path_len) {
    // Not fully implemented in browser environment
    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: path_symlink
  path_symlink(old_path, old_path_len, fd, new_path, new_path_len) {
    // Not implemented in browser environment
    return WASI.ERRNO_NOSYS;
  }

  // WASI implementation: path_unlink_file
  path_unlink_file(fd, path, path_len) {
    // Not fully implemented in browser environment
    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: poll_oneoff
  poll_oneoff(in_ptr, out_ptr, nsubscriptions, nevents) {
    // Basic implementation that always returns immediately
    this.refreshMemory();
    this.view.setUint32(nevents, 0, true);
    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: proc_exit
  proc_exit(rval) {
    // We can't really exit in a browser environment
    throw new Error(`WASI: process exited with code ${rval}`);
  }

  // WASI implementation: proc_raise
  proc_raise(sig) {
    // Not implemented in browser environment
    return WASI.ERRNO_NOSYS;
  }

  // WASI implementation: random_get
  random_get(buf, buf_len) {
    this.refreshMemory();

    const buffer = new Uint8Array(this.memory.buffer, buf, buf_len);
    crypto.getRandomValues(buffer);

    return WASI.ERRNO_SUCCESS;
  }

  // WASI implementation: sched_yield
  sched_yield() {
    // No-op in browser environment
    return WASI.ERRNO_SUCCESS;
  }

  // Helper to resolve a path
  _resolvePath(basePath, relativePath) {
    if (relativePath.startsWith("/")) {
      return relativePath;
    }

    if (basePath === "/") {
      return "/" + relativePath;
    }

    return basePath + "/" + relativePath;
  }

  // Helper to create a virtual file
  createVirtualFile(path, content) {
    return this.fs.writeFile(path, content);
  }

  // Helper to read a virtual file
  readVirtualFile(path) {
    const data = this.fs.readFile(path);
    return data ? new TextDecoder().decode(data) : null;
  }
}

// Export the WASI implementation
window.WASI = {
  WASIImplementation,
  ERRNO: WASI,
};
