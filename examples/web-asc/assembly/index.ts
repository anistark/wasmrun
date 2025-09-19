// Simple DOM manipulation functions
// Helper function for debug logging (console.log equivalent)
function debugLog(message: string): void {
  // In a web environment, this would use console.log
  // For now, we'll just format the message
}

export function greet(name: string): string {
  debugLog("[WEB-ASC] ===== AssemblyScript Web Example Starting =====");
  debugLog("[WEB-ASC] greet() function called with name: " + name + " (running from web-asc)");
  const result = `Hello, ${name} from AssemblyScript WebApp!`;
  debugLog("[WEB-ASC] greet() generated result: " + result);
  debugLog("[WEB-ASC] greet() function completed");
  return result;
}

export function createButton(text: string, id: string): string {
  debugLog("[WEB-ASC] createButton() function called with text: " + text + ", id: " + id + " (running from web-asc)");
  const result = `<button id="${id}" onclick="handleClick('${id}')">${text}</button>`;
  debugLog("[WEB-ASC] createButton() generated HTML: " + result);
  debugLog("[WEB-ASC] createButton() function completed");
  return result;
}

export function fibonacci(n: i32): i32 {
  debugLog("[WEB-ASC] fibonacci() function called with n: " + n.toString() + " (running from web-asc)");
  if (n <= 1) {
    debugLog("[WEB-ASC] fibonacci() base case reached, returning " + n.toString());
    return n;
  }
  debugLog("[WEB-ASC] fibonacci() calculating recursively for n=" + n.toString());
  const result = fibonacci(n - 1) + fibonacci(n - 2);
  debugLog("[WEB-ASC] fibonacci(" + n.toString() + ") calculated result: " + result.toString());
  return result;
}

export function isPrime(n: i32): bool {
  debugLog("[WEB-ASC] isPrime() function called with n: " + n.toString() + " (running from web-asc)");
  if (n <= 1) {
    debugLog("[WEB-ASC] isPrime() n<=1, returning false (not prime)");
    return false;
  }
  if (n <= 3) {
    debugLog("[WEB-ASC] isPrime() n<=3, returning true (prime)");
    return true;
  }
  if (n % 2 === 0 || n % 3 === 0) {
    debugLog("[WEB-ASC] isPrime() divisible by 2 or 3, returning false (not prime)");
    return false;
  }

  debugLog("[WEB-ASC] isPrime() checking divisibility from 5 onwards");
  for (let i: i32 = 5; i * i <= n; i += 6) {
    debugLog("[WEB-ASC] isPrime() checking divisors " + i.toString() + " and " + (i+2).toString());
    if (n % i === 0 || n % (i + 2) === 0) {
      debugLog("[WEB-ASC] isPrime() found divisor, returning false (not prime)");
      return false;
    }
  }

  debugLog("[WEB-ASC] isPrime() completed: " + n.toString() + " is prime");
  return true;
}

export function reverseString(input: string): string {
  debugLog("[WEB-ASC] reverseString() function called with input: " + input + " (running from web-asc)");
  let result = "";
  for (let i = input.length - 1; i >= 0; i--) {
    result += input.charAt(i);
  }
  debugLog("[WEB-ASC] reverseString() result: " + result);
  debugLog("[WEB-ASC] reverseString() function completed");
  return result;
}

export function power(base: f64, exponent: i32): f64 {
  debugLog("[WEB-ASC] power() function called with base: " + base.toString() + ", exponent: " + exponent.toString() + " (running from web-asc)");
  if (exponent === 0) {
    debugLog("[WEB-ASC] power() exponent is 0, returning 1.0");
    return 1.0;
  }
  if (exponent < 0) {
    debugLog("[WEB-ASC] power() negative exponent, calculating reciprocal");
    return 1.0 / power(base, -exponent);
  }

  let result: f64 = 1.0;
  for (let i = 0; i < exponent; i++) {
    result *= base;
    debugLog("[WEB-ASC] power() step " + i.toString() + ": result=" + result.toString());
  }
  debugLog("[WEB-ASC] power() final result: " + result.toString());
  debugLog("[WEB-ASC] power() function completed");
  return result;
}

export function validateEmail(email: string): bool {
  debugLog("[WEB-ASC] validateEmail() function called with email: " + email + " (running from web-asc)");
  const hasAt = email.includes("@");
  const hasDot = email.includes(".");
  const isValid = hasAt && hasDot;
  debugLog("[WEB-ASC] validateEmail() hasAt: " + hasAt.toString() + ", hasDot: " + hasDot.toString());
  debugLog("[WEB-ASC] validateEmail() result: " + isValid.toString());
  debugLog("[WEB-ASC] validateEmail() function completed");
  return isValid;
}

export function calculateArea(shape: string, width: f64, height: f64): f64 {
  debugLog("[WEB-ASC] calculateArea() function called with shape: " + shape + ", width: " + width.toString() + ", height: " + height.toString() + " (running from web-asc)");
  let result: f64 = 0.0;
  if (shape === "rectangle") {
    debugLog("[WEB-ASC] calculateArea() calculating rectangle area");
    result = width * height;
  } else if (shape === "triangle") {
    debugLog("[WEB-ASC] calculateArea() calculating triangle area");
    result = 0.5 * width * height;
  } else if (shape === "circle") {
    debugLog("[WEB-ASC] calculateArea() calculating circle area (width as radius)");
    result = 3.14159 * width * width; // width as radius
  } else {
    debugLog("[WEB-ASC] calculateArea() unknown shape, returning 0.0");
  }
  debugLog("[WEB-ASC] calculateArea() calculated result: " + result.toString());
  debugLog("[WEB-ASC] calculateArea() function completed");
  return result;
}