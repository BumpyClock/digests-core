ABOUTME: Workspace overview for digests-core shared parsing library.
ABOUTME: Documents crates, FFI, and platform usage patterns.

# digests-core

Rust workspace providing shared parsing primitives and a C ABI for multi-platform apps:

- `crates/feed`: feed parsing (RSS/Atom/podcast) → DFeed ABI.
- `crates/hermes`: ReaderView/article extraction and metadata (Hermes port) → DReaderView / DMetadata.
- `crates/ffi`: C ABI surface over the parsers with arena-managed results.
- `crates/cli`: developer CLI for feed parsing.

## Building

```bash
cargo test -q    # run all tests
cargo build -p digests-ffi --release   # produce FFI library (libdigests_ffi.*)
```

## FFI Usage (C ABI)

Functions (blocking):

- `digests_extract_reader(url_ptr, url_len, html_ptr, html_len, out_err) -> DReaderArena*`
- `digests_reader_result(arena) -> const DReaderView*`
- `digests_free_reader(arena)`
- `digests_extract_metadata(html_ptr, html_len, base_url_ptr, base_url_len, out_err) -> DMetaArena*`
- `digests_metadata_result(arena) -> const DMetadata*`
- `digests_free_metadata(arena)`

All strings are UTF-8 slices (`ptr+len`, not null-terminated). Results live in an arena; free the arena when done. On success `out_err->code == D_OK`.

## Platform Helpers (async wrappers)

FFI calls are synchronous; wrap them off the main thread:

### Swift
```swift
// Suppose you expose C functions via module map.
func extractReaderAsync(url: String, html: Data, completion: @escaping (DReaderView?, DError) -> Void) {
    DispatchQueue.global(qos: .userInitiated).async {
        var err = DError(code: D_OK, message: DString(data: nil, len: 0))
        let arena = html.withUnsafeBytes { bytes in
            url.withCString { cUrl in
                digests_extract_reader(
                    UnsafeRawPointer(cUrl), url.utf8.count,
                    bytes.baseAddress, html.count,
                    &err
                )
            }
        }
        let view = arena.flatMap { digests_reader_result($0) }
        DispatchQueue.main.async {
            completion(view?.pointee, err)
            if let arena = arena { digests_free_reader(arena) }
        }
    }
}
```

### Kotlin (JNI)
```kotlin
fun extractReaderAsync(url: String, html: ByteArray, callback: (ReaderView?, Int) -> Unit) {
    CoroutineScope(Dispatchers.IO).launch {
        val err = DError()
        val arena = digests_extract_reader(url, html, err)
        val view = arena?.let { digests_reader_result(it) }
        withContext(Dispatchers.Main) {
            callback(view, err.code)
            arena?.let { digests_free_reader(it) }
        }
    }
}
```

### C# (P/Invoke)
```csharp
public static Task<ReaderView?> ExtractReaderAsync(string url, byte[] html) =>
    Task.Run(() =>
    {
        var err = new DError();
        var arena = digests_extract_reader(url, html, ref err);
        var view = arena != IntPtr.Zero ? Marshal.PtrToStructure<ReaderView>(digests_reader_result(arena)) : (ReaderView?)null;
        if (arena != IntPtr.Zero) digests_free_reader(arena);
        return view;
    });
```

Adjust signatures to your actual FFI bindings; key point is to call the blocking C function on a background thread/queue/dispatcher, then marshal results back to the UI thread.

## Metadata-only

`digests_extract_metadata` is inexpensive; still treat it as blocking and wrap similarly if calling from UI threads.

## Notes

- Strings are not null-terminated; always use `len`.
- Arenas own all returned memory; do not free individual pointers.
- Check `DIGESTS_FFI_VERSION` for ABI compatibility in your bindings.
