#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>

// Export functions for WebAssembly
#ifdef __EMSCRIPTEN__
#include <emscripten.h>
#define EXPORT EMSCRIPTEN_KEEPALIVE
#else
#define EXPORT
#endif

// Simple greeting function
EXPORT
void greet(const char* name) {
    printf("Hello, %s! This is a C WebAssembly example.\n", name);
}

// Fibonacci calculation
EXPORT
int fibonacci(int n) {
    if (n <= 1) return n;
    return fibonacci(n - 1) + fibonacci(n - 2);
}

// Sum array elements
EXPORT
int sum_array(int* numbers, int length) {
    int sum = 0;
    for (int i = 0; i < length; i++) {
        sum += numbers[i];
    }
    printf("Sum of array: %d\n", sum);
    return sum;
}

// Calculate factorial
EXPORT
long long factorial(int n) {
    if (n <= 1) return 1;
    long long result = 1;
    for (int i = 2; i <= n; i++) {
        result *= i;
    }
    printf("factorial(%d) = %lld\n", n, result);
    return result;
}

// Prime number check
EXPORT
int is_prime(int n) {
    if (n <= 1) return 0;
    if (n <= 3) return 1;
    if (n % 2 == 0 || n % 3 == 0) return 0;
    
    for (int i = 5; i * i <= n; i += 6) {
        if (n % i == 0 || n % (i + 2) == 0) return 0;
    }
    
    printf("%d is %s\n", n, "prime");
    return 1;
}

// String length function
EXPORT
int string_length(const char* str) {
    int length = strlen(str);
    printf("Length of '%s': %d\n", str, length);
    return length;
}

// Memory allocation example
EXPORT
int* create_array(int size) {
    int* arr = (int*)malloc(size * sizeof(int));
    if (arr) {
        // Initialize with sequential values
        for (int i = 0; i < size; i++) {
            arr[i] = i + 1;
        }
        printf("Created array of size %d\n", size);
    }
    return arr;
}

// Free allocated memory
EXPORT
void free_array(int* arr) {
    if (arr) {
        free(arr);
        printf("Memory freed\n");
    }
}

// Square root calculation
EXPORT
double square_root(double x) {
    double result = sqrt(x);
    printf("sqrt(%.2f) = %.6f\n", x, result);
    return result;
}

// Main function for initialization
int main() {
    printf("🔧 C WebAssembly module loaded!\n");
    printf("Available functions:\n");
    printf("- greet(name)\n");
    printf("- fibonacci(n)\n");
    printf("- sum_array(numbers, length)\n");
    printf("- factorial(n)\n");
    printf("- is_prime(n)\n");
    printf("- string_length(str)\n");
    printf("- create_array(size)\n");
    printf("- free_array(arr)\n");
    printf("- square_root(x)\n");
    
    // Example usage
    printf("\n--- Example Usage ---\n");
    greet("World");
    printf("fibonacci(10) = %d\n", fibonacci(10));
    factorial(5);
    is_prime(17);
    square_root(25.0);
    
    return 0;
}