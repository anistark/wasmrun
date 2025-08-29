# C WebAssembly Makefile for Emscripten

CC=emcc
CFLAGS=-O2 -s WASM=1 -s EXPORTED_FUNCTIONS="['_greet','_fibonacci','_sum_array','_factorial','_is_prime','_string_length','_create_array','_free_array','_square_root','_main']" -s EXPORTED_RUNTIME_METHODS="['ccall','cwrap']" -s ALLOW_MEMORY_GROWTH=1
SOURCE=main.c
OUTPUT=main

.PHONY: all clean

all: $(OUTPUT).wasm

$(OUTPUT).wasm: $(SOURCE)
	$(CC) $(CFLAGS) $(SOURCE) -o $(OUTPUT).js

clean:
	rm -f $(OUTPUT).js $(OUTPUT).wasm

# Helper target for wasmrun compatibility
wasm: all