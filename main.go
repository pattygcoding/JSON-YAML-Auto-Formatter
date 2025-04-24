// main.go
package main

import (
	"bytes"
	"encoding/json"
	"strings"
	"syscall/js"

	"gopkg.in/yaml.v3"
)

func formatInput(this js.Value, args []js.Value) interface{} {
	if len(args) == 0 {
		return js.ValueOf("No input provided.")
	}

	input := args[0].String()
	input = strings.TrimSpace(input)

	forceMode := ""
	if strings.HasPrefix(input, "///force:") {
		newlineIndex := strings.Index(input, "\n")
		if newlineIndex == -1 {
			// If input is single-line, strip out the force tag
			newlineIndex = strings.Index(input, "}///") + 1
		}
		if strings.Contains(input, "///force:json///") {
			forceMode = "json"
			input = strings.Replace(input, "///force:json///", "", 1)
		} else if strings.Contains(input, "///force:yaml///") {
			forceMode = "yaml"
			input = strings.Replace(input, "///force:yaml///", "", 1)
		}
		input = strings.TrimSpace(input)
	}

	if forceMode == "yaml" || (forceMode == "" && isYAML(input)) {
		result, err := formatYAML(input)
		if err != nil {
			return js.ValueOf("Invalid YAML: " + err.Error())
		}
		return js.ValueOf(result)
	} else {
		result, err := formatJSON(input)
		if err != nil {
			return js.ValueOf("Invalid JSON: " + err.Error())
		}
		return js.ValueOf(result)
	}
}

func formatJSON(input string) (string, error) {
	var parsed interface{}
	if err := json.Unmarshal([]byte(input), &parsed); err != nil {
		return "", err
	}

	buf := new(bytes.Buffer)
	encoder := json.NewEncoder(buf)
	encoder.SetIndent("", "    ")
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(parsed); err != nil {
		return "", err
	}

	return strings.TrimSpace(buf.String()), nil
}

func formatYAML(input string) (string, error) {
	var parsed interface{}
	if err := yaml.Unmarshal([]byte(input), &parsed); err != nil {
		return "", err
	}

	buf := new(bytes.Buffer)
	d := yaml.NewEncoder(buf)
	d.SetIndent(4)
	if err := d.Encode(parsed); err != nil {
		return "", err
	}
	return strings.TrimSpace(buf.String()), nil
}

func isYAML(input string) bool {
	if strings.HasPrefix(input, "---") || strings.Contains(input, ":") && !strings.HasPrefix(input, "{") {
		return true
	}
	return false
}

func main() {
	js.Global().Set("formatJSON", js.FuncOf(formatInput))
	select {}
}
