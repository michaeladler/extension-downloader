package pathutil

import (
	"os/user"
	"path/filepath"
)

func ExpandUser(path string) string {
	if path != "" && path[0] == '~' {
		usr, err := user.Current()
		if err != nil {
			return path
		}
		return filepath.Join(usr.HomeDir, path[1:])
	}
	return path
}
