"""
Python WebAssembly Example using Pyodide

This example demonstrates basic Python functionality that can be 
compiled to WebAssembly and run in the browser.
"""

import json
import math
from typing import List


def greet(name: str) -> str:
    """Greet a person with a personalized message."""
    message = f"Hello, {name}! This is a Python WebAssembly example."
    print(message)
    return message


def fibonacci(n: int) -> int:
    """Calculate the nth Fibonacci number."""
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)


def sum_array(numbers: List[int]) -> int:
    """Sum all numbers in a list."""
    total = sum(numbers)
    print(f"Sum of {numbers}: {total}")
    return total


def calculate_pi(iterations: int = 1000) -> float:
    """Calculate Pi using the Leibniz formula."""
    pi_approximation = 0
    for i in range(iterations):
        pi_approximation += ((-1) ** i) / (2 * i + 1)
    pi_approximation *= 4
    print(f"Pi approximation with {iterations} iterations: {pi_approximation}")
    return pi_approximation


def process_json(json_string: str) -> dict:
    """Parse JSON string and return processed data."""
    try:
        data = json.loads(json_string)
        processed = {
            "original": data,
            "keys": list(data.keys()) if isinstance(data, dict) else None,
            "length": len(data) if isinstance(data, (list, dict, str)) else None,
            "type": type(data).__name__
        }
        print(f"Processed JSON: {processed}")
        return processed
    except json.JSONDecodeError as e:
        error_msg = f"JSON parsing error: {e}"
        print(error_msg)
        return {"error": error_msg}


def math_operations(x: float, y: float) -> dict:
    """Perform various math operations on two numbers."""
    result = {
        "addition": x + y,
        "subtraction": x - y,
        "multiplication": x * y,
        "division": x / y if y != 0 else float('inf'),
        "power": x ** y,
        "square_root_x": math.sqrt(abs(x)),
        "sin_x": math.sin(x),
        "cos_x": math.cos(x)
    }
    print(f"Math operations on {x} and {y}: {result}")
    return result


if __name__ == "__main__":
    print("🐍 Python WebAssembly module loaded!")
    print("Available functions:")
    print("- greet(name)")
    print("- fibonacci(n)")
    print("- sum_array(numbers)")
    print("- calculate_pi(iterations)")
    print("- process_json(json_string)")
    print("- math_operations(x, y)")
    
    # Example usage
    print("\n--- Example Usage ---")
    greet("World")
    print(f"fibonacci(10) = {fibonacci(10)}")
    print(f"sum_array([1, 2, 3, 4, 5]) = {sum_array([1, 2, 3, 4, 5])}")
    calculate_pi(100)
    math_operations(10, 3)