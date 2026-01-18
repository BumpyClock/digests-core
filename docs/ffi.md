# FFI (C ABI) Interface

The `ffi` crate provides a C-compatible interface for the digests-core parsers, allowing embedding in other languages.

## Architecture

The FFI interface uses an **arena-based memory management** pattern:
1. **Arenas** allocate memory for results
2. **Pointers** reference data within arenas
3. **Cleanup functions** free arena memory

## Quick Start

### C Header
```c
// digests_ffi.h
typedef struct DReaderArena DReaderArena;
typedef struct DReaderView DReaderView;
typedef struct DMetaArena DMetaArena;
typedef struct DMetadata DMetadata;

// Reader view extraction
DReaderArena* digests_extract_reader(
    const char* url,
    size_t url_len,
    const char* html,
    size_t html_len,
    char** out_err
);

// Access reader view
const DReaderView* digests_reader_result(DReaderArena* arena);
void digests_free_reader(DReaderArena* arena);

// Metadata extraction
DMetaArena* digests_extract_metadata(
    const char* html,
    size_t html_len,
    const char* base_url,
    size_t base_url_len,
    char** out_err
);

// Access metadata
const DMetadata* digests_metadata_result(DMetaArena* arena);
void digests_free_metadata(DMetaArena* arena);
```

### Usage Example
```c
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "digests_ffi.h"

int main() {
    const char* url = "https://example.com/article";
    const char* html = "<html><body><article><h1>Title</h1><p>Content</p></article></body></html>";

    // Extract reader view
    char* err = NULL;
    DReaderArena* arena = digests_extract_reader(url, strlen(url), html, strlen(html), &err);

    if (err) {
        fprintf(stderr, "Error: %s\n", err);
        free(err);
        return 1;
    }

    // Access results
    const DReaderView* view = digests_reader_result(arena);
    printf("Title: %s\n", view->title);
    printf("Content: %s\n", view->content);

    // Cleanup
    digests_free_reader(arena);
    return 0;
}
```

## Data Structures

### DReaderArena
Arena allocator for reader view results:
```c
typedef struct DReaderArena DReaderArena;
// No direct fields - use accessor functions
```

### DReaderView
Extracted article content:
```c
typedef struct DReaderView {
    const char* title;
    const char* content;
    size_t length;
    const char* excerpt;
    const char* site_name;
    const char* author;
    const char* published_date;
    const char* language;
    size_t reading_time;
    float confidence;
} DReaderView;
```

### DMetaArena
Arena allocator for metadata results:
```c
typedef struct DMetaArena DMetaArena;
// No direct fields - use accessor functions
```

### DMetadata
Article metadata structure:
```c
typedef struct DMetadata {
    const char* title;
    const char* author;
    const char* published_date;
    const char* excerpt;
    const char* language;
    const char** keywords;
    size_t keyword_count;
    const char* description;
    const char* site_name;
    const char* url;
    const char* image_url;
    const char* favicon_url;
    const char* canonical_url;
    // OpenGraph and Twitter card data
    // (see advanced section)
} DMetadata;
```

## Platform Support

### Building the Library
```bash
# Linux
cargo build -p digests-ffi --release
# Produces: target/release/libdigests_ffi.so

# macOS
cargo build -p digests-ffi --release
# Produces: target/release/libdigests_ffi.dylib

# Windows
cargo build -p digests-ffi --release
# Produces: target/release/digests_ffi.dll
```

### Loading the Library

#### Linux/macOS
```c
void* lib = dlopen("./libdigests_ffi.so", RTLD_LAZY);
if (!lib) {
    fprintf(stderr, "Error loading library: %s\n", dlerror());
}

// Load function pointers
typedef DReaderArena* (*ExtractReaderFunc)(const char*, size_t, const char*, size_t, char**);
ExtractReaderFunc digests_extract_reader = dlsym(lib, "digests_extract_reader");
```

#### Windows
```c
HINSTANCE lib = LoadLibrary("digests_ffi.dll");
if (!lib) {
    fprintf(stderr, "Error loading library\n");
}

typedef DReaderArena* (*ExtractReaderFunc)(const char*, size_t, const char*, size_t, char**);
ExtractReaderFunc digests_extract_reader = (ExtractReaderFunc)GetProcAddress(lib, "digests_extract_reader");
```

## Language Bindings

### Python
```python
import ctypes
import ctypes.util

# Load library
lib = ctypes.CDLL("./libdigests_ffi.so")

# Define function signatures
lib.digests_extract_reader.argtypes = [
    ctypes.c_char_p, ctypes.c_size_t,
    ctypes.c_char_p, ctypes.c_size_t,
    ctypes.POINTER(ctypes.c_char_p)
]
lib.digests_extract_reader.restype = ctypes.c_void_p

# Use
url = "https://example.com"
html = "<html>...</html>"
err = ctypes.c_char_p()
arena = lib.digests_extract_reader(
    url.encode(), len(url),
    html.encode(), len(html),
    ctypes.byref(err)
)

if err.value:
    print("Error:", err.value.decode())
    exit(1)

# Access results
view = lib.digests_reader_result(arena)
print("Title:", ctypes.c_char_p(view.contents.title).value.decode())
lib.digests_free_reader(arena)
```

### Node.js (Native Addon)
```javascript
const ffi = require('ffi-napi');

const lib = ffi.Library('./libdigests_ffi', {
    'digests_extract_reader': [
        'pointer', [
            'string', 'size_t',
            'string', 'size_t',
            'pointer'
        ]
    ],
    'digests_reader_result': ['pointer', ['pointer']],
    'digests_free_reader': ['void', ['pointer']]
});

// Use
const url = "https://example.com";
const html = "<html>...</html>";
const errPtr = ['string'];
const arena = lib.digests_extract_reader(url, url.length, html, html.length, errPtr);

if (errPtr[0]) {
    console.error("Error:", errPtr[0]);
    process.exit(1);
}

const view = lib.digests_reader_result(arena);
console.log("Title:", view.title);
lib.digests_free_reader(arena);
```

### Go
```go
package main

/*
#cgo LDFLAGS: -L. -ldigests_ffi
#include <stdlib.h>
#include "digests_ffi.h"
*/
import "C"
import "unsafe"

func main() {
    url := C.CString("https://example.com")
    defer C.free(unsafe.Pointer(url))

    html := C.CString("<html>...</html>")
    defer C.free(unsafe.Pointer(html))

    var err *C.char
    arena := C.digests_extract_reader(url, C.size_t(len("https://example.com")),
                                       html, C.size_t(len("<html>...</html>")), &err)

    if err != nil {
        println("Error:", C.GoString(err))
        C.free(unsafe.Pointer(err))
        return
    }

    view := C.digests_reader_result(arena)
    println("Title:", C.GoString(view.title))

    C.digests_free_reader(arena)
}
```

## Memory Management

### Arena Pattern
1. **Allocation**: Results are allocated in an arena
2. **Access**: Use accessor functions to get pointers
3. **Deallocation**: Free the entire arena at once

### Example
```c
// Extract (allocates arena)
DReaderArena* arena = digests_extract_reader(url, url_len, html, html_len, &err);

// Access (returns pointers within arena)
const DReaderView* view = digests_reader_result(arena);

// Use data
printf("Title: %s\n", view->title);
printf("Content length: %zu\n", view->length);

// Free everything
digests_free_reader(arena);  // Frees arena and all contained data
```

## Error Handling

### Error String Pattern
Functions that can fail take an `out_err` parameter:
```c
// Input parameters
const char* url, size_t url_len,
const char* html, size_t html_len,
char** out_err

// Usage
char* err = NULL;
DReaderArena* arena = digests_extract_reader(url, url_len, html, html_len, &err);

if (err) {
    // Error occurred
    fprintf(stderr, "Error: %s\n", err);
    free(err);
    return 1;
}

// Success
// ... use arena ...
```

### Error Codes
Common error patterns:
- NULL pointer in input
- Invalid UTF-8
- Empty HTML content
- Network-related errors (when fetching)

## Advanced Features

### Metadata Access
```c
// Extract metadata
DMetaArena* meta_arena = digests_extract_metadata(html, html_len, base_url, base_url_len, &err);
const DMetadata* metadata = digests_metadata_result(meta_arena);

// Access fields
if (metadata->author) {
    printf("Author: %s\n", metadata->author);
}

// Access keywords
for (size_t i = 0; i < metadata->keyword_count; i++) {
    printf("Keyword: %s\n", metadata->keywords[i]);
}

// Cleanup
digests_free_metadata(meta_arena);
```

### Array Handling
For arrays like keywords:
```c
const char** keywords = metadata->keywords;
size_t count = metadata->keyword_count;

for (size_t i = 0; i < count; i++) {
    printf("Keyword %zu: %s\n", i, keywords[i]);
}
```

## Performance Tips

1. **Reuse arenas** when extracting multiple articles
2. **Batch processing** for better cache utilization
3. **Minimize string copies** in calling code
4. **Use appropriate timeouts** to prevent hanging

## Testing

### C Unit Tests
```c
#include <assert.h>

void test_extraction() {
    const char* simple_html = "<html><body><h1>Title</h1><p>Content</p></body></html>";

    char* err = NULL;
    DReaderArena* arena = digests_extract_reader(
        "https://example.com", 15,
        simple_html, strlen(simple_html),
        &err
    );

    assert(!err);
    assert(arena != NULL);

    const DReaderView* view = digests_reader_result(arena);
    assert(strcmp(view->title, "Title") == 0);

    digests_free_reader(arena);
}
```

### Integration Tests
```c
void test_real_site() {
    // Fetch real HTML and test extraction
    // Use curl or similar to get HTML
    char* html = fetch_html("https://example.com/article");

    // Test extraction
    // ...

    free(html);
}
```

## Troubleshooting

### Common Issues

1. **Linker errors**: Ensure library is built and linked correctly
2. **Memory leaks**: Always call free functions
3. **Invalid UTF-8**: Ensure input strings are valid UTF-8
4. **NULL pointers**: Check all input parameters

### Debug Mode
Enable debug output:
```bash
export DIGESTS_FFI_DEBUG=1
```

### Platform Notes

#### Linux
- Install development packages: `libssl-dev`
- Use `ldd` to check dependencies

#### macOS
- Ensure Xcode command line tools are installed
- Handle library search paths with `-L`

#### Windows
- Use Visual Studio Build Tools
- Handle DLL search paths in application