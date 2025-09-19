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
    print(f"[PYTHON-HELLO] greet() function called with name: {name} (running from python-hello)")
    message = f"Hello, {name}! This is a Python WebAssembly example."
    print(f"[PYTHON-HELLO] greet() generated message: {message}")
    print(message)
    print("[PYTHON-HELLO] greet() function completed")
    return message


def fibonacci(n: int) -> int:
    """Calculate the nth Fibonacci number."""
    print(f"[PYTHON-HELLO] fibonacci() function called with n: {n} (running from python-hello)")
    if n <= 1:
        print(f"[PYTHON-HELLO] fibonacci() base case reached, returning {n}")
        return n
    print(f"[PYTHON-HELLO] fibonacci() calculating recursively for n={n}")
    result = fibonacci(n - 1) + fibonacci(n - 2)
    print(f"[PYTHON-HELLO] fibonacci({n}) calculated result: {result}")
    return result


def sum_array(numbers: List[int]) -> int:
    """Sum all numbers in a list."""
    print(f"[PYTHON-HELLO] sum_array() function called with {len(numbers)} numbers (running from python-hello)")
    print(f"[PYTHON-HELLO] sum_array() processing array: {numbers}")
    total = sum(numbers)
    print(f"[PYTHON-HELLO] sum_array() calculated total: {total}")
    print(f"Sum of {numbers}: {total}")
    print("[PYTHON-HELLO] sum_array() function completed")
    return total


def calculate_pi(iterations: int = 1000) -> float:
    """Calculate Pi using the Leibniz formula."""
    print(f"[PYTHON-HELLO] calculate_pi() function called with iterations: {iterations} (running from python-hello)")
    print("[PYTHON-HELLO] calculate_pi() using Leibniz formula")
    pi_approximation = 0
    for i in range(iterations):
        term = ((-1) ** i) / (2 * i + 1)
        pi_approximation += term
        if i % 100 == 0:  # Log every 100th iteration to avoid spam
            print(f"[PYTHON-HELLO] calculate_pi() iteration {i}: current approximation = {pi_approximation * 4}")
    pi_approximation *= 4
    print(f"[PYTHON-HELLO] calculate_pi() final result: {pi_approximation}")
    print(f"Pi approximation with {iterations} iterations: {pi_approximation}")
    print("[PYTHON-HELLO] calculate_pi() function completed")
    return pi_approximation


def process_json(json_string: str) -> dict:
    """Parse JSON string and return processed data."""
    print(f"[PYTHON-HELLO] process_json() function called (running from python-hello)")
    print(f"[PYTHON-HELLO] process_json() input string length: {len(json_string)}")
    try:
        print("[PYTHON-HELLO] process_json() attempting to parse JSON")
        data = json.loads(json_string)
        print(f"[PYTHON-HELLO] process_json() successfully parsed data type: {type(data).__name__}")
        processed = {
            "original": data,
            "keys": list(data.keys()) if isinstance(data, dict) else None,
            "length": len(data) if isinstance(data, (list, dict, str)) else None,
            "type": type(data).__name__
        }
        print(f"[PYTHON-HELLO] process_json() created processed object")
        print(f"Processed JSON: {processed}")
        print("[PYTHON-HELLO] process_json() function completed successfully")
        return processed
    except json.JSONDecodeError as e:
        error_msg = f"JSON parsing error: {e}"
        print(f"[PYTHON-HELLO] process_json() ERROR: {error_msg}")
        print(error_msg)
        return {"error": error_msg}


def math_operations(x: float, y: float) -> dict:
    """Perform various math operations on two numbers."""
    print(f"[PYTHON-HELLO] math_operations() function called with x={x}, y={y} (running from python-hello)")
    print("[PYTHON-HELLO] math_operations() performing calculations")
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
    print(f"[PYTHON-HELLO] math_operations() calculations completed")
    if y == 0:
        print("[PYTHON-HELLO] math_operations() WARNING: division by zero handled")
    print(f"Math operations on {x} and {y}: {result}")
    print("[PYTHON-HELLO] math_operations() function completed")
    return result


if __name__ == "__main__":
    print("[PYTHON-HELLO] ===== Python WebAssembly Example Starting =====")
    print("[PYTHON-HELLO] Initializing Python-Hello example module")
    print("[PYTHON-HELLO] Module: Python-Hello Example (python-hello/main.py)")
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
    print("[PYTHON-HELLO] Running example function calls")
    greet("World")
    print("[PYTHON-HELLO] Calling fibonacci(10)")
    print(f"fibonacci(10) = {fibonacci(10)}")
    print("[PYTHON-HELLO] Calling sum_array([1, 2, 3, 4, 5])")
    print(f"sum_array([1, 2, 3, 4, 5]) = {sum_array([1, 2, 3, 4, 5])}")
    print("[PYTHON-HELLO] Calling calculate_pi(100)")
    calculate_pi(100)
    print("[PYTHON-HELLO] Calling math_operations(10, 3)")
    math_operations(10, 3)
    print("[PYTHON-HELLO] ===== Python WebAssembly Example Completed =====")