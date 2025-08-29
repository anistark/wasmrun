// AssemblyScript WebAssembly Example

// Import console for logging (available in AssemblyScript)
declare function consoleLog(ptr: usize): void;

// Export a simple greeting function
export function greet(name: string): string {
  const message = `Hello, ${name}! This is an AssemblyScript WebAssembly example.`;
  consoleLog(changetype<usize>(message));
  return message;
}

// Export a fibonacci function
export function fibonacci(n: i32): i32 {
  if (n <= 1) return n;
  return fibonacci(n - 1) + fibonacci(n - 2);
}

// Export a function to sum array elements
export function sumArray(arr: i32[]): i32 {
  let sum: i32 = 0;
  for (let i = 0; i < arr.length; i++) {
    sum += arr[i];
  }
  return sum;
}

// Export a function to check if number is prime
export function isPrime(n: i32): bool {
  if (n <= 1) return false;
  if (n <= 3) return true;
  if (n % 2 === 0 || n % 3 === 0) return false;
  
  for (let i: i32 = 5; i * i <= n; i += 6) {
    if (n % i === 0 || n % (i + 2) === 0) return false;
  }
  
  return true;
}

// Export a function to calculate factorial
export function factorial(n: i32): i64 {
  if (n <= 1) return 1;
  let result: i64 = 1;
  for (let i: i32 = 2; i <= n; i++) {
    result *= i64(i);
  }
  return result;
}

// Export a function to reverse a string
export function reverseString(str: string): string {
  let reversed = "";
  for (let i = str.length - 1; i >= 0; i--) {
    reversed += str.charAt(i);
  }
  return reversed;
}

// Export a function to find maximum in array
export function findMax(arr: i32[]): i32 {
  if (arr.length === 0) return 0;
  
  let max = arr[0];
  for (let i = 1; i < arr.length; i++) {
    if (arr[i] > max) {
      max = arr[i];
    }
  }
  return max;
}

// Export a function to calculate power
export function power(base: f64, exponent: f64): f64 {
  return Math.pow(base, exponent);
}

// Export a function to calculate square root
export function squareRoot(x: f64): f64 {
  return Math.sqrt(x);
}

// Memory management example - create an array
export function createArray(size: i32, value: i32): i32[] {
  const arr = new Array<i32>(size);
  for (let i = 0; i < size; i++) {
    arr[i] = value;
  }
  return arr;
}