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
    printf("[C-HELLO] greet() function called with name: %s (running from c-hello)\n", name);
    printf("Hello, %s! This is a C WebAssembly example. (running from c-hello)\n", name);
    printf("[C-HELLO] greet() function completed\n");
}

// Fibonacci calculation
EXPORT
int fibonacci(int n) {
    printf("[C-HELLO] fibonacci() function called with n=%d (running from c-hello)\n", n);
    if (n <= 1) {
        printf("[C-HELLO] fibonacci() base case reached, returning %d\n", n);
        return n;
    }
    printf("[C-HELLO] fibonacci() calculating recursively for n=%d\n", n);
    int result = fibonacci(n - 1) + fibonacci(n - 2);
    printf("[C-HELLO] fibonacci(%d) calculated result: %d\n", n, result);
    return result;
}

// Sum array elements
EXPORT
int sum_array(int* numbers, int length) {
    printf("[C-HELLO] sum_array() function called with length=%d (running from c-hello)\n", length);
    int sum = 0;
    for (int i = 0; i < length; i++) {
        printf("[C-HELLO] sum_array() processing element %d: %d\n", i, numbers[i]);
        sum += numbers[i];
    }
    printf("[C-HELLO] sum_array() completed. Sum of array: %d\n", sum);
    return sum;
}

// Calculate factorial
EXPORT
long long factorial(int n) {
    printf("[C-HELLO] factorial() function called with n=%d (running from c-hello)\n", n);
    if (n <= 1) {
        printf("[C-HELLO] factorial() base case, returning 1\n");
        return 1;
    }
    long long result = 1;
    for (int i = 2; i <= n; i++) {
        result *= i;
        printf("[C-HELLO] factorial() step %d: result=%lld\n", i, result);
    }
    printf("[C-HELLO] factorial(%d) completed with result: %lld\n", n, result);
    return result;
}

// Prime number check
EXPORT
int is_prime(int n) {
    printf("[C-HELLO] is_prime() function called with n=%d (running from c-hello)\n", n);
    if (n <= 1) {
        printf("[C-HELLO] is_prime() n<=1, returning 0 (not prime)\n");
        return 0;
    }
    if (n <= 3) {
        printf("[C-HELLO] is_prime() n<=3, returning 1 (prime)\n");
        return 1;
    }
    if (n % 2 == 0 || n % 3 == 0) {
        printf("[C-HELLO] is_prime() divisible by 2 or 3, returning 0 (not prime)\n");
        return 0;
    }

    printf("[C-HELLO] is_prime() checking divisibility from 5 onwards\n");
    for (int i = 5; i * i <= n; i += 6) {
        printf("[C-HELLO] is_prime() checking divisors %d and %d\n", i, i+2);
        if (n % i == 0 || n % (i + 2) == 0) {
            printf("[C-HELLO] is_prime() found divisor, returning 0 (not prime)\n");
            return 0;
        }
    }

    printf("[C-HELLO] is_prime() completed: %d is prime\n", n);
    return 1;
}

// String length function
EXPORT
int string_length(const char* str) {
    printf("[C-HELLO] string_length() function called with str='%s' (running from c-hello)\n", str);
    int length = strlen(str);
    printf("[C-HELLO] string_length() calculated length: %d\n", length);
    printf("Length of '%s': %d\n", str, length);
    return length;
}

// Memory allocation example
EXPORT
int* create_array(int size) {
    printf("[C-HELLO] create_array() function called with size=%d (running from c-hello)\n", size);
    int* arr = (int*)malloc(size * sizeof(int));
    if (arr) {
        printf("[C-HELLO] create_array() memory allocated successfully\n");
        // Initialize with sequential values
        for (int i = 0; i < size; i++) {
            arr[i] = i + 1;
            printf("[C-HELLO] create_array() initialized arr[%d] = %d\n", i, arr[i]);
        }
        printf("[C-HELLO] create_array() array initialization completed\n");
        printf("Created array of size %d\n", size);
    } else {
        printf("[C-HELLO] create_array() ERROR: memory allocation failed\n");
    }
    return arr;
}

// Free allocated memory
EXPORT
void free_array(int* arr) {
    printf("[C-HELLO] free_array() function called (running from c-hello)\n");
    if (arr) {
        printf("[C-HELLO] free_array() freeing memory\n");
        free(arr);
        printf("[C-HELLO] free_array() memory freed successfully\n");
        printf("Memory freed\n");
    } else {
        printf("[C-HELLO] free_array() WARNING: null pointer passed\n");
    }
}

// Square root calculation
EXPORT
double square_root(double x) {
    printf("[C-HELLO] square_root() function called with x=%.2f (running from c-hello)\n", x);
    if (x < 0) {
        printf("[C-HELLO] square_root() WARNING: negative input\n");
    }
    double result = sqrt(x);
    printf("[C-HELLO] square_root() calculated result: %.6f\n", result);
    printf("sqrt(%.2f) = %.6f\n", x, result);
    return result;
}

// Main function for initialization
int main() {
    printf("[C-HELLO] ===== C WebAssembly Example Starting =====\n");
    printf("[C-HELLO] Initializing C-Hello example module\n");
    printf("🔧 C WebAssembly module loaded!\n");
    printf("[C-HELLO] Module: C-Hello Example (c-hello/main.c)\n");
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
    printf("[C-HELLO] Running example function calls\n");
    greet("World");
    printf("[C-HELLO] Calling fibonacci(10)\n");
    printf("fibonacci(10) = %d\n", fibonacci(10));
    printf("[C-HELLO] Calling factorial(5)\n");
    factorial(5);
    printf("[C-HELLO] Calling is_prime(17)\n");
    is_prime(17);
    printf("[C-HELLO] Calling square_root(25.0)\n");
    square_root(25.0);
    printf("[C-HELLO] ===== C WebAssembly Example Completed =====\n");

    return 0;
}