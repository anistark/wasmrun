package main

import (
	"fmt"
	"syscall/js"
)

// greet function callable from JavaScript
func greet(this js.Value, p []js.Value) interface{} {
	name := p[0].String()
	message := fmt.Sprintf("Hello, %s! This is a Go WebAssembly example.", name)
	fmt.Println(message)
	return message
}

// fibonacci function callable from JavaScript
func fibonacci(this js.Value, p []js.Value) interface{} {
	n := p[0].Int()
	result := fib(n)
	fmt.Printf("fibonacci(%d) = %d\n", n, result)
	return result
}

// Helper function for fibonacci calculation
func fib(n int) int {
	if n <= 1 {
		return n
	}
	return fib(n-1) + fib(n-2)
}

// sumArray function callable from JavaScript
func sumArray(this js.Value, p []js.Value) interface{} {
	arr := p[0]
	length := arr.Get("length").Int()
	sum := 0
	
	for i := 0; i < length; i++ {
		sum += arr.Index(i).Int()
	}
	
	fmt.Printf("Sum of array: %d\n", sum)
	return sum
}

// getCurrentTime function callable from JavaScript
func getCurrentTime(this js.Value, p []js.Value) interface{} {
	now := js.Global().Get("Date").New()
	timeStr := now.Call("toISOString").String()
	fmt.Printf("Current time: %s\n", timeStr)
	return timeStr
}

func main() {
	fmt.Println("🐹 Go WebAssembly module loaded!")
	fmt.Println("Available functions: greet(), fibonacci(), sumArray(), getCurrentTime()")
	
	// Register functions to be called from JavaScript
	js.Global().Set("greet", js.FuncOf(greet))
	js.Global().Set("fibonacci", js.FuncOf(fibonacci))
	js.Global().Set("sumArray", js.FuncOf(sumArray))
	js.Global().Set("getCurrentTime", js.FuncOf(getCurrentTime))
	
	// Keep the program running
	select {}
}