def greet(name: str) -> str:
    """Greet someone by name."""
    return "Hello, " + name + "!"

def add(a: int, b: int) -> int:
    """Add two numbers."""
    return a + b

def fibonacci(n: int) -> int:
    """Calculate fibonacci number."""
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)

def factorial(n: int) -> int:
    """Calculate factorial."""
    if n <= 1:
        return 1
    return n * factorial(n - 1)
