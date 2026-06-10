package main

import "example.com/sample/greet"

func main() {
	g := greet.New("world")
	_ = g.Hello()
}
