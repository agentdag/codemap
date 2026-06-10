package greet

// Greeter greets people.
type Greeter struct {
	Name string
}

// Speaker is anything that can speak.
type Speaker interface {
	Speak() string
}

// Message is an alias for a plain string.
type Message = string

// New builds a Greeter.
func New(name string) *Greeter {
	return &Greeter{Name: name}
}

func (g *Greeter) Hello() string {
	return "hello, " + g.Name
}

func (g Greeter) Bye() Message {
	return "bye, " + g.Name
}
