package formatter

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestFixtures(t *testing.T) {
	cases := []struct {
		file    string
		format  string
		invalid bool
	}{
		{"json/valid.json", "json", false},
		{"json/invalid.json", "json", true},
		{"yaml/valid.yaml", "yaml", false},
		{"yaml/invalid.yaml", "yaml", true},
		{"yaml/duplicate-keys.yaml", "yaml", true},
		{"yaml/multiple-documents.yaml", "yaml", false},
		{"yaml/list.yaml", "yaml", false},
	}
	for _, test := range cases {
		for _, mode := range []string{"auto", test.format} {
			t.Run(test.file+"/"+mode, func(t *testing.T) {
				input, err := os.ReadFile(filepath.Join("..", "..", "test", test.file))
				if err != nil {
					t.Fatal(err)
				}
				result, err := Format(string(input), mode)
				if test.invalid {
					if err == nil || !strings.Contains(err.Error(), "invalid "+strings.ToUpper(test.format)) {
						t.Fatalf("expected invalid %s, got %v", test.format, err)
					}
					return
				}
				if err != nil || result.Format != test.format {
					t.Fatalf("result = %+v, err = %v", result, err)
				}
				repeated, err := Format(result.Text, mode)
				if err != nil || repeated != result {
					t.Fatalf("formatting is not stable: %+v, %v", repeated, err)
				}
			})
		}
	}
}

func TestJSONIndentationAndPrecision(t *testing.T) {
	result, err := Format(`{"nested":{"number":9007199254740993},"html":"<b>"}`, "auto")
	want := "{\n    \"nested\": {\n        \"number\": 9007199254740993\n    },\n    \"html\": \"<b>\"\n}"
	if err != nil || result.Text != want || result.Format != "json" {
		t.Fatalf("result = %+v, err = %v; want %q", result, err, want)
	}
}

func TestDetection(t *testing.T) {
	cases := []struct {
		input  string
		format string
	}{
		{`"https://example.com"`, "json"},
		{`[{"name":"test"}]`, "json"},
		{`[1,2,3]`, "json"},
		{`true`, "json"},
		{`null`, "json"},
		{`42`, "json"},
		{`hello`, "yaml"},
		{`"name": test`, "yaml"},
		{`{name: test, items: [one, two]}`, "yaml"},
		{`[one, two]`, "yaml"},
		{"- one\n- two", "yaml"},
		{"---\nname: test", "yaml"},
	}
	for _, test := range cases {
		t.Run(test.input, func(t *testing.T) {
			result, err := Format(test.input, "auto")
			if err != nil || result.Format != test.format {
				t.Fatalf("result = %+v, err = %v; want %s", result, err, test.format)
			}
		})
	}
}

func TestInvalidInput(t *testing.T) {
	for _, input := range []string{"", "  \n", `{"name":}`, `[{"name":}]`, `[1,2,]`, `[true,false,]`, "name: first\n---\nitems: [broken", "name: *missing", "# comment only"} {
		t.Run(input, func(t *testing.T) {
			if result, err := Format(input, "auto"); err == nil {
				t.Fatalf("expected error, got %+v", result)
			}
		})
	}
}

func TestForcedModes(t *testing.T) {
	if _, err := Format("name: test", "json"); err == nil {
		t.Fatal("YAML accepted as JSON")
	}
	result, err := Format(`{"name":"test"}`, "yaml")
	if err != nil || result.Format != "yaml" {
		t.Fatalf("forced YAML result = %+v, err = %v", result, err)
	}
	if _, err := Format("name: test", "xml"); err == nil {
		t.Fatal("unknown mode accepted")
	}
}

func TestYAMLPreservation(t *testing.T) {
	input := "# settings\ndefaults: &defaults\n  enabled: true\ncopy: *defaults\nmessage: |\n  first\n  second\n---\nname: second"
	result, err := Format(input, "auto")
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{"# settings", "&defaults", "*defaults", "\n    enabled: true", "message: |\n    first\n    second", "\n---\nname: second"} {
		if !strings.Contains(result.Text, want) {
			t.Errorf("missing %q in %q", want, result.Text)
		}
	}
}
