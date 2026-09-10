package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"gopkg.in/yaml.v3"
)

type record struct {
	ID       int64             `json:"id" yaml:"id"`
	Name     string            `json:"name" yaml:"name"`
	Enabled  bool              `json:"enabled" yaml:"enabled"`
	Tags     []string          `json:"tags" yaml:"tags"`
	Metadata map[string]string `json:"metadata" yaml:"metadata"`
	Samples  []float64         `json:"samples" yaml:"samples"`
	Notes    string            `json:"notes" yaml:"notes"`
	Optional interface{}       `json:"optional" yaml:"optional"`
}

func main() {
	if _, err := os.Stat("go.mod"); err != nil {
		panic("run this generator from the repository root")
	}
	directory := filepath.Join("test", "stress")
	if err := os.MkdirAll(directory, 0755); err != nil {
		panic(err)
	}
	write := func(name string, content []byte) {
		if err := os.WriteFile(filepath.Join(directory, name), content, 0644); err != nil {
			panic(err)
		}
		fmt.Printf("%-32s %10d bytes\n", name, len(content))
	}
	records := make([]record, 3000)
	for index := range records {
		records[index] = record{
			ID:      9007199254740993 + int64(index),
			Name:    fmt.Sprintf("service-%04d", index),
			Enabled: index%3 != 0,
			Tags:    []string{"production", "stress-test", fmt.Sprintf("region-%d", index%12)},
			Metadata: map[string]string{
				"url":   fmt.Sprintf("https://example.com/api/items/%d?active=true&limit=100", index),
				"path":  `C:\data\imports\records.json`,
				"owner": fmt.Sprintf("team-%02d", index%40),
			},
			Samples: []float64{0, 1.25, -3.5, float64(index) / 8, 1000000.125},
			Notes:   "First line with a colon: value and # hash.\nSecond line with \"quotes\", a tab\tand <html>.\n",
		}
	}
	jsonData, err := json.Marshal(map[string]interface{}{"name": "large-record-set", "records": records})
	if err != nil {
		panic(err)
	}
	write("large-valid.json", jsonData)
	write("large-invalid.json", jsonData[:len(jsonData)-1])
	yamlData, err := yaml.Marshal(map[string]interface{}{"name": "large-record-set", "records": records})
	if err != nil {
		panic(err)
	}
	write("large-valid.yaml", yamlData)
	write("large-invalid.yaml", append(append([]byte{}, yamlData...), []byte("broken: [unterminated\n")...))

	var nested interface{} = map[string]interface{}{"leaf": "deepest", "number": int64(9007199254740993)}
	for depth := 0; depth < 128; depth++ {
		nested = map[string]interface{}{"level": depth, "children": []interface{}{nested}, "padding": "nested JSON stress fixture"}
	}
	deepData, err := json.Marshal(nested)
	if err != nil {
		panic(err)
	}
	write("deep-valid.json", deepData)

	var stream bytes.Buffer
	encoder := yaml.NewEncoder(&stream)
	encoder.SetIndent(2)
	for index := 0; index < 300; index++ {
		if err := encoder.Encode(records[index]); err != nil {
			panic(err)
		}
	}
	if err := encoder.Close(); err != nil {
		panic(err)
	}
	write("stream-valid.yaml", stream.Bytes())
}
