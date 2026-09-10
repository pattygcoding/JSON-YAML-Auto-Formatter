package formatter

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestStressFixtures(t *testing.T) {
	files, err := filepath.Glob(filepath.Join("..", "..", "test", "stress", "*.*"))
	if err != nil {
		t.Fatal(err)
	}
	count := 0
	for _, file := range files {
		format := strings.TrimPrefix(filepath.Ext(file), ".")
		if format != "json" && format != "yaml" {
			continue
		}
		count++
		for _, mode := range []string{"auto", format} {
			t.Run(filepath.Base(file)+"/"+mode, func(t *testing.T) {
				input, err := os.ReadFile(file)
				if err != nil {
					t.Fatal(err)
				}
				started := time.Now()
				result, err := Format(string(input), mode)
				t.Logf("%d input bytes, %d output bytes, %s", len(input), len(result.Text), time.Since(started))
				if strings.Contains(filepath.Base(file), "-invalid.") {
					if err == nil || !strings.HasPrefix(err.Error(), "invalid "+strings.ToUpper(format)+":") {
						t.Fatalf("expected invalid %s error, got %v", format, err)
					}
					return
				}
				if err != nil || result.Format != format {
					t.Fatalf("detected %q, error %v", result.Format, err)
				}
				if !strings.Contains(result.Text, "9007199254740993") {
					t.Fatal("large integer was lost")
				}
				repeated, err := Format(result.Text, mode)
				if err != nil || repeated != result {
					t.Fatalf("formatting is not stable: %v", err)
				}
			})
		}
	}
	if count != 6 {
		t.Fatalf("expected 6 stress fixtures, found %d; run go run ./scripts/generate-stress.go", count)
	}
}
