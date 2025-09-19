package main

import (
	"fmt"
	"syscall/js"
)

// greet function callable from JavaScript
func greet(this js.Value, p []js.Value) interface{} {
	fmt.Println("[GO-HELLO] greet() function called (running from go-hello)")
	name := p[0].String()
	fmt.Printf("[GO-HELLO] greet() received name: %s (running from go-hello)\n", name)
	message := fmt.Sprintf("Hello, %s! This is a Go WebAssembly example.", name)
	fmt.Printf("[GO-HELLO] greet() generated message: %s\n", message)
	fmt.Println(message)
	fmt.Println("[GO-HELLO] greet() function completed")
	return message
}

// fibonacci function callable from JavaScript
func fibonacci(this js.Value, p []js.Value) interface{} {
	fmt.Println("[GO-HELLO] fibonacci() function called (running from go-hello)")
	n := p[0].Int()
	fmt.Printf("[GO-HELLO] fibonacci() received n: %d (running from go-hello)\n", n)
	fmt.Printf("[GO-HELLO] fibonacci() calling helper function fib(%d)\n", n)
	result := fib(n)
	fmt.Printf("[GO-HELLO] fibonacci() calculated result: %d\n", result)
	fmt.Printf("fibonacci(%d) = %d\n", n, result)
	fmt.Println("[GO-HELLO] fibonacci() function completed")
	return result
}

// Helper function for fibonacci calculation
func fib(n int) int {
	fmt.Printf("[GO-HELLO] fib() helper called with n=%d (running from go-hello)\n", n)
	if n <= 1 {
		fmt.Printf("[GO-HELLO] fib() base case reached, returning %d\n", n)
		return n
	}
	fmt.Printf("[GO-HELLO] fib() calculating recursively for n=%d\n", n)
	result := fib(n-1) + fib(n-2)
	fmt.Printf("[GO-HELLO] fib(%d) recursive result: %d\n", n, result)
	return result
}

// sumArray function callable from JavaScript
func sumArray(this js.Value, p []js.Value) interface{} {
	fmt.Println("[GO-HELLO] sumArray() function called (running from go-hello)")
	arr := p[0]
	length := arr.Get("length").Int()
	fmt.Printf("[GO-HELLO] sumArray() processing array of length: %d\n", length)
	sum := 0

	for i := 0; i < length; i++ {
		value := arr.Index(i).Int()
		fmt.Printf("[GO-HELLO] sumArray() processing element %d: %d\n", i, value)
		sum += value
	}

	fmt.Printf("[GO-HELLO] sumArray() calculated total: %d\n", sum)
	fmt.Printf("Sum of array: %d\n", sum)
	fmt.Println("[GO-HELLO] sumArray() function completed")
	return sum
}

// getCurrentTime function callable from JavaScript
func getCurrentTime(this js.Value, p []js.Value) interface{} {
	fmt.Println("[GO-HELLO] getCurrentTime() function called (running from go-hello)")
	fmt.Println("[GO-HELLO] getCurrentTime() accessing browser Date API")
	now := js.Global().Get("Date").New()
	timeStr := now.Call("toISOString").String()
	fmt.Printf("[GO-HELLO] getCurrentTime() retrieved time: %s\n", timeStr)
	fmt.Printf("Current time: %s\n", timeStr)
	fmt.Println("[GO-HELLO] getCurrentTime() function completed")
	return timeStr
}

func main() {
	fmt.Println("[GO-HELLO] ===== Go WebAssembly Example Starting =====")
	fmt.Println("[GO-HELLO] Initializing Go-Hello example module")
	fmt.Println("[GO-HELLO] Module: Go-Hello Example (go-hello/main.go)")
	fmt.Println("🐹 Go WebAssembly module loaded!")
	fmt.Println("Available functions: greet(), fibonacci(), sumArray(), getCurrentTime()")
	fmt.Println("[GO-HELLO] Registering JavaScript-callable functions")
	
	// Register functions to be called from JavaScript
	fmt.Println("[GO-HELLO] Registering function: greet")
	js.Global().Set("greet", js.FuncOf(greet))
	fmt.Println("[GO-HELLO] Registering function: fibonacci")
	js.Global().Set("fibonacci", js.FuncOf(fibonacci))
	fmt.Println("[GO-HELLO] Registering function: sumArray")
	js.Global().Set("sumArray", js.FuncOf(sumArray))
	fmt.Println("[GO-HELLO] Registering function: getCurrentTime")
	js.Global().Set("getCurrentTime", js.FuncOf(getCurrentTime))

	fmt.Println("[GO-HELLO] All functions registered successfully")
	fmt.Println("[GO-HELLO] ===== Go WebAssembly Example Ready =====")
	// Keep the program running
	select {}
}