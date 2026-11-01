// Command inventory parses syntax without loading or executing the application.
package main

import (
	"bytes"
	"encoding/json"
	"go/ast"
	"go/format"
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

type Entry struct {
	Path        string `json:"path"`
	Line        int    `json:"line"`
	Kind        string `json:"kind"`
	Name        string `json:"name"`
	Declaration string `json:"declaration"`
}

func main() {
	root := os.Args[1]
	var result []Entry
	err := filepath.WalkDir(root, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			if strings.HasPrefix(d.Name(), ".") && path != root {
				return filepath.SkipDir
			}
			return nil
		}
		if !strings.HasSuffix(path, ".go") {
			return nil
		}
		set := token.NewFileSet()
		file, err := parser.ParseFile(set, path, nil, parser.ParseComments)
		if err != nil {
			return err
		}
		rel, _ := filepath.Rel(root, path)
		render := func(n ast.Node) string {
			var b bytes.Buffer
			if e := format.Node(&b, set, n); e != nil {
				panic(e)
			}
			return b.String()
		}
		add := func(kind, name string, n ast.Node) {
			result = append(result, Entry{filepath.ToSlash(rel), set.Position(n.Pos()).Line, kind, name, func() string {
				if f, ok := n.(*ast.Field); ok {
					return render(&ast.StructType{Fields: &ast.FieldList{List: []*ast.Field{f}}})
				}
				return render(n)
			}()})
		}
		for _, decl := range file.Decls {
			switch n := decl.(type) {
			case *ast.FuncDecl:
				if n.Recv != nil && len(n.Recv.List) == 1 && render(n.Recv.List[0].Type) == "*App" && ast.IsExported(n.Name.Name) {
					saved := n.Body
					n.Body = nil
					add("application-command", n.Name.Name, n)
					n.Body = saved
				}
				if strings.HasSuffix(path, "_test.go") && strings.HasPrefix(n.Name.Name, "Test") {
					add("go-test", n.Name.Name, n.Type)
				}
			case *ast.GenDecl:
				for _, spec := range n.Specs {
					if t, ok := spec.(*ast.TypeSpec); ok {
						if st, ok := t.Type.(*ast.StructType); ok {
							for _, f := range st.Fields.List {
								if f.Tag != nil {
									tag, _ := strconv.Unquote(f.Tag.Value)
									if strings.Contains(tag, "json:") {
										names := []string{}
										for _, name := range f.Names {
											names = append(names, name.Name)
										}
										add("serialized-field", t.Name.Name+"."+strings.Join(names, ","), f)
									}
								}
							}
						}
					}
				}
			}
		}
		return nil
	})
	if err != nil {
		panic(err)
	}
	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	if err := enc.Encode(result); err != nil {
		panic(err)
	}
}
