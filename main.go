//go:build js && wasm

package main

import (
	"strings"
	"syscall/js"

	"formatter/internal/formatter"
)

func formatInput(this js.Value, args []js.Value) interface{} {
	if len(args) == 0 || args[0].Type() != js.TypeString {
		return "No input provided."
	}
	input := strings.TrimSpace(args[0].String())
	mode := "auto"
	if len(args) > 1 && args[1].Type() == js.TypeString {
		mode = args[1].String()
	}
	for _, forced := range []string{"json", "yaml"} {
		prefix := "///force:" + forced + "///"
		if strings.HasPrefix(input, prefix) {
			mode = forced
			input = strings.TrimPrefix(input, prefix)
			break
		}
	}
	result, err := formatter.Format(input, mode)
	if err != nil {
		return err.Error()
	}
	return result.Text
}

func main() {
	callback := js.FuncOf(formatInput)
	js.Global().Set("formatInput", callback)
	js.Global().Set("formatJSON", callback)
	select {}
}
