package main

import (
	"fmt"

	"example.com/sample/greet"
)

func main() {
	g := greet.New("world")
	fmt.Println(g.Hello())
	fmt.Println(g.Bye())
}
