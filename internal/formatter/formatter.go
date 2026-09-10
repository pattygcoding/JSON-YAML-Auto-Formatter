package formatter

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"strings"

	"gopkg.in/yaml.v3"
)

type Result struct {
	Text   string
	Format string
}

func Format(input, mode string) (Result, error) {
	input = strings.TrimSpace(input)
	if input == "" {
		return Result{}, fmt.Errorf("no input provided")
	}
	if mode == "" || mode == "auto" {
		if json.Valid([]byte(input)) || looksLikeJSON(input) {
			mode = "json"
		} else {
			mode = "yaml"
		}
	}

	var text string
	var err error
	switch mode {
	case "json":
		text, err = formatJSON(input)
	case "yaml":
		text, err = formatYAML(input)
	default:
		return Result{}, fmt.Errorf("unknown format %q", mode)
	}
	if err != nil {
		return Result{}, fmt.Errorf("invalid %s: %w", strings.ToUpper(mode), err)
	}
	return Result{Text: text, Format: mode}, nil
}

func looksLikeJSON(input string) bool {
	remaining := input
	for strings.HasPrefix(remaining, "[") {
		remaining = strings.TrimSpace(remaining[1:])
	}
	if strings.HasPrefix(remaining, "{") {
		remaining = strings.TrimSpace(remaining[1:])
		return remaining == "" || strings.HasPrefix(remaining, `"`) || strings.HasPrefix(remaining, "}")
	}
	if remaining == input {
		return false
	}
	if remaining == "" || strings.ContainsAny(remaining[:1], `"-0123456789]`) {
		return true
	}
	for _, literal := range []string{"true", "false", "null"} {
		if remaining == literal || strings.HasPrefix(remaining, literal+",") || strings.HasPrefix(remaining, literal+"]") || strings.HasPrefix(remaining, literal+" ") {
			return true
		}
	}
	return false
}

func formatJSON(input string) (string, error) {
	var output bytes.Buffer
	if err := json.Indent(&output, []byte(input), "", "    "); err != nil {
		return "", err
	}
	return output.String(), nil
}

func formatYAML(input string) (string, error) {
	decoder := yaml.NewDecoder(strings.NewReader(input))
	var output bytes.Buffer
	encoder := yaml.NewEncoder(&output)
	encoder.SetIndent(4)
	documents := 0
	for {
		var document yaml.Node
		if err := decoder.Decode(&document); err == io.EOF {
			break
		} else if err != nil {
			return "", err
		}
		var validated interface{}
		if err := document.Decode(&validated); err != nil {
			return "", err
		}
		if err := encoder.Encode(&document); err != nil {
			return "", err
		}
		documents++
	}
	if documents == 0 {
		return "", fmt.Errorf("no YAML document provided")
	}
	if err := encoder.Close(); err != nil {
		return "", err
	}
	return strings.TrimSuffix(output.String(), "\n"), nil
}
