// Pure WASI Go example - simple exported functions only
// Compile with: tinygo build -target wasi -o main.wasm main.go
// Run with: wasmrun exec main.wasm -c add 5 3

package main

// Add two numbers
//go:export add
func add(a int32, b int32) int32 {
	return a + b
}

// Multiply two numbers
//go:export multiply
func multiply(a int32, b int32) int32 {
	return a * b
}

// Calculate fibonacci
//go:export fibonacci
func fibonacci(n uint32) uint32 {
	if n == 0 {
		return 0
	}
	if n == 1 {
		return 1
	}
	return fibonacci(n-1) + fibonacci(n-2)
}

// Power function
//go:export power
func power(base int32, exp uint32) int32 {
	result := int32(1)
	for i := uint32(0); i < exp; i++ {
		result *= base
	}
	return result
}

// Check if even
//go:export is_even
func is_even(n int32) int32 {
	if n%2 == 0 {
		return 1
	}
	return 0
}

// Check if prime
//go:export is_prime
func is_prime(n int32) int32 {
	if n < 2 {
		return 0
	}
	if n == 2 {
		return 1
	}
	if n%2 == 0 {
		return 0
	}

	for i := int32(3); i*i <= n; i += 2 {
		if n%i == 0 {
			return 0
		}
	}
	return 1
}

func main() {
	// Empty main - all functionality is in exported functions
}
