// AssemblyScript WebAssembly Example

// Import console for logging (available in AssemblyScript)
declare function consoleLog(ptr: usize): void;

// Helper function for debug logging
function debugLog(message: string): void {
  consoleLog(changetype<usize>(message));
}

// Export a simple greeting function
export function greet(name: string): string {
  debugLog("[ASC-HELLO] ===== AssemblyScript WebAssembly Example Starting =====");
  debugLog("[ASC-HELLO] greet() function called with name: " + name + " (running from asc-hello)");
  const message = `Hello, ${name}! This is an AssemblyScript WebAssembly example.`;
  debugLog("[ASC-HELLO] greet() generated message: " + message);
  consoleLog(changetype<usize>(message));
  debugLog("[ASC-HELLO] greet() function completed");
  return message;
}

// Export a fibonacci function
export function fibonacci(n: i32): i32 {
  debugLog("[ASC-HELLO] fibonacci() function called with n: " + n.toString() + " (running from asc-hello)");
  if (n <= 1) {
    debugLog("[ASC-HELLO] fibonacci() base case reached, returning " + n.toString());
    return n;
  }
  debugLog("[ASC-HELLO] fibonacci() calculating recursively for n=" + n.toString());
  const result = fibonacci(n - 1) + fibonacci(n - 2);
  debugLog("[ASC-HELLO] fibonacci(" + n.toString() + ") calculated result: " + result.toString());
  return result;
}

// Export a function to sum array elements
export function sumArray(arr: i32[]): i32 {
  debugLog("[ASC-HELLO] sumArray() function called with " + arr.length.toString() + " elements (running from asc-hello)");
  let sum: i32 = 0;
  for (let i = 0; i < arr.length; i++) {
    debugLog("[ASC-HELLO] sumArray() processing element " + i.toString() + ": " + arr[i].toString());
    sum += arr[i];
  }
  debugLog("[ASC-HELLO] sumArray() calculated total: " + sum.toString());
  debugLog("[ASC-HELLO] sumArray() function completed");
  return sum;
}

// Export a function to check if number is prime
export function isPrime(n: i32): bool {
  debugLog("[ASC-HELLO] isPrime() function called with n: " + n.toString() + " (running from asc-hello)");
  if (n <= 1) {
    debugLog("[ASC-HELLO] isPrime() n<=1, returning false (not prime)");
    return false;
  }
  if (n <= 3) {
    debugLog("[ASC-HELLO] isPrime() n<=3, returning true (prime)");
    return true;
  }
  if (n % 2 === 0 || n % 3 === 0) {
    debugLog("[ASC-HELLO] isPrime() divisible by 2 or 3, returning false (not prime)");
    return false;
  }

  debugLog("[ASC-HELLO] isPrime() checking divisibility from 5 onwards");
  for (let i: i32 = 5; i * i <= n; i += 6) {
    debugLog("[ASC-HELLO] isPrime() checking divisors " + i.toString() + " and " + (i+2).toString());
    if (n % i === 0 || n % (i + 2) === 0) {
      debugLog("[ASC-HELLO] isPrime() found divisor, returning false (not prime)");
      return false;
    }
  }

  debugLog("[ASC-HELLO] isPrime() completed: " + n.toString() + " is prime");
  return true;
}

// Export a function to calculate factorial
export function factorial(n: i32): i64 {
  debugLog("[ASC-HELLO] factorial() function called with n: " + n.toString() + " (running from asc-hello)");
  if (n <= 1) {
    debugLog("[ASC-HELLO] factorial() base case, returning 1");
    return 1;
  }
  let result: i64 = 1;
  for (let i: i32 = 2; i <= n; i++) {
    result *= i64(i);
    debugLog("[ASC-HELLO] factorial() step " + i.toString() + ": result=" + result.toString());
  }
  debugLog("[ASC-HELLO] factorial(" + n.toString() + ") completed with result: " + result.toString());
  return result;
}

// Export a function to reverse a string
export function reverseString(str: string): string {
  debugLog("[ASC-HELLO] reverseString() function called with string: " + str + " (running from asc-hello)");
  let reversed = "";
  for (let i = str.length - 1; i >= 0; i--) {
    reversed += str.charAt(i);
  }
  debugLog("[ASC-HELLO] reverseString() result: " + reversed);
  debugLog("[ASC-HELLO] reverseString() function completed");
  return reversed;
}

// Export a function to find maximum in array
export function findMax(arr: i32[]): i32 {
  debugLog("[ASC-HELLO] findMax() function called with " + arr.length.toString() + " elements (running from asc-hello)");
  if (arr.length === 0) {
    debugLog("[ASC-HELLO] findMax() empty array, returning 0");
    return 0;
  }

  let max = arr[0];
  debugLog("[ASC-HELLO] findMax() initial max: " + max.toString());
  for (let i = 1; i < arr.length; i++) {
    if (arr[i] > max) {
      debugLog("[ASC-HELLO] findMax() new max found at index " + i.toString() + ": " + arr[i].toString());
      max = arr[i];
    }
  }
  debugLog("[ASC-HELLO] findMax() final result: " + max.toString());
  debugLog("[ASC-HELLO] findMax() function completed");
  return max;
}

// Export a function to calculate power
export function power(base: f64, exponent: f64): f64 {
  debugLog("[ASC-HELLO] power() function called with base: " + base.toString() + ", exponent: " + exponent.toString() + " (running from asc-hello)");
  const result = Math.pow(base, exponent);
  debugLog("[ASC-HELLO] power() calculated result: " + result.toString());
  debugLog("[ASC-HELLO] power() function completed");
  return result;
}

// Export a function to calculate square root
export function squareRoot(x: f64): f64 {
  debugLog("[ASC-HELLO] squareRoot() function called with x: " + x.toString() + " (running from asc-hello)");
  const result = Math.sqrt(x);
  debugLog("[ASC-HELLO] squareRoot() calculated result: " + result.toString());
  debugLog("[ASC-HELLO] squareRoot() function completed");
  return result;
}

// Memory management example - create an array
export function createArray(size: i32, value: i32): i32[] {
  debugLog("[ASC-HELLO] createArray() function called with size: " + size.toString() + ", value: " + value.toString() + " (running from asc-hello)");
  const arr = new Array<i32>(size);
  debugLog("[ASC-HELLO] createArray() array allocated");
  for (let i = 0; i < size; i++) {
    arr[i] = value;
    if (i < 5) { // Only log first 5 elements to avoid spam
      debugLog("[ASC-HELLO] createArray() initialized arr[" + i.toString() + "] = " + value.toString());
    }
  }
  debugLog("[ASC-HELLO] createArray() array initialization completed");
  debugLog("[ASC-HELLO] createArray() function completed");
  return arr;
}