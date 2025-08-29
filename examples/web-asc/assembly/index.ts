// Simple DOM manipulation functions
export function greet(name: string): string {
  return `Hello, ${name} from AssemblyScript WebApp!`;
}

export function createButton(text: string, id: string): string {
  return `<button id="${id}" onclick="handleClick('${id}')">${text}</button>`;
}

export function fibonacci(n: i32): i32 {
  if (n <= 1) return n;
  return fibonacci(n - 1) + fibonacci(n - 2);
}

export function isPrime(n: i32): bool {
  if (n <= 1) return false;
  if (n <= 3) return true;
  if (n % 2 === 0 || n % 3 === 0) return false;
  
  for (let i: i32 = 5; i * i <= n; i += 6) {
    if (n % i === 0 || n % (i + 2) === 0) {
      return false;
    }
  }
  return true;
}

export function reverseString(input: string): string {
  let result = "";
  for (let i = input.length - 1; i >= 0; i--) {
    result += input.charAt(i);
  }
  return result;
}

export function power(base: f64, exponent: i32): f64 {
  if (exponent === 0) return 1.0;
  if (exponent < 0) return 1.0 / power(base, -exponent);
  
  let result: f64 = 1.0;
  for (let i = 0; i < exponent; i++) {
    result *= base;
  }
  return result;
}

export function validateEmail(email: string): bool {
  return email.includes("@") && email.includes(".");
}

export function calculateArea(shape: string, width: f64, height: f64): f64 {
  if (shape === "rectangle") {
    return width * height;
  } else if (shape === "triangle") {
    return 0.5 * width * height;
  } else if (shape === "circle") {
    return 3.14159 * width * width; // width as radius
  }
  return 0.0;
}