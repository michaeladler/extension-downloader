package main

import (
	"flag"
	"fmt"
	"os"
	"path"
	"sync"
	"sync/atomic"
	"time"

	"log/slog"

	"github.com/lmittmann/tint"
	"github.com/michaeladler/extension-downloader/internal/chromium"
	"github.com/michaeladler/extension-downloader/internal/config"
	"github.com/michaeladler/extension-downloader/internal/firefox"
	"github.com/michaeladler/extension-downloader/internal/pathutil"
	"github.com/spf13/afero"
)

type ChromiumResult struct {
	Id    string
	Fname string
}

type FirefoxResult struct {
	Name  string
	Fname string
}

// filled in by linker
var Version, Commit, Date string

var (
	versionFlag  bool
	logLevelFlag string
)

func init() {
	flag.BoolVar(&versionFlag, "version", false, "Prints the version and exit")
	flag.StringVar(&logLevelFlag, "log-level", slog.LevelInfo.String(), "Set the log level")
}

func main() {
	flag.Parse()
	os.Exit(_main(afero.NewOsFs()))
}

func _main(fs afero.Fs) int {
	if versionFlag {
		fmt.Printf("extension-downloader version=%s, commit=%s, date=%s\n", Version, Commit, Date)
		return 0
	}

	opts := tint.Options{AddSource: true}
	if logLevelFlag != "" {
		lvl := new(slog.Level)
		if err := lvl.UnmarshalText([]byte(logLevelFlag)); err != nil {
			fmt.Fprintln(os.Stderr, "Failed to parse log level:", logLevelFlag)
			return 1
		}
		opts.Level = lvl
	}
	logger := slog.New(tint.NewHandler(os.Stdout, &opts))
	slog.SetDefault(logger)

	commit := Commit[0:min(len(Commit), 7)]
	slog.Info("extension-downloader starting", "version", Version, "commit", commit, "date", Date)

	configPath := pathutil.ExpandUser("~/.config/extension-downloader/config.toml")
	cfg, err := config.LoadConfig(fs, configPath)
	if err != nil {
		slog.Error("Failed to load config file", "configPath", configPath, "err", err)
		return 1
	}

	extensionsDir := os.Getenv("XDG_DATA_HOME")
	if extensionsDir == "" {
		extensionsDir = path.Join(os.Getenv("HOME"), ".local", "share")
	}
	extensionsDir = path.Join(extensionsDir, "extension-downloader")
	extensionsDirFirefox := path.Join(extensionsDir, "firefox")
	extensionsDirChromium := path.Join(extensionsDir, "chromium")
	_ = fs.MkdirAll(extensionsDirFirefox, 0755)
	_ = fs.MkdirAll(extensionsDirChromium, 0755)

	var errCount atomic.Int32

	// collect ids to fetch
	chromiumToProfiles := make(map[string][]string, 32)
	firefoxToProfiles := make(map[string][]string, 32)

	start := time.Now()
	for _, extCfg := range cfg.Extensions {
		profile := extCfg.Profile
		for _, name := range extCfg.Names {
			if extCfg.Browser == config.FIREFOX {
				firefoxToProfiles[name] = append(firefoxToProfiles[name], profile)
			} else if extCfg.Browser == config.CHROMIUM {
				chromiumToProfiles[name] = append(chromiumToProfiles[name], profile)
			} else {
				slog.Error("Unsupported browser", "browser", extCfg.Browser)
				return 1
			}
		}
	}

	var firefoxWg sync.WaitGroup
	firefoxResults := make(chan FirefoxResult)
	for name := range firefoxToProfiles {
		name := name
		// capture loop variable
		firefoxWg.Add(1)
		go func() {
			defer firefoxWg.Done()
			fname, err := firefox.DownloadExtension(fs, name, extensionsDirFirefox)
			if err != nil {
				slog.Error("Failed to download Firefox extension", "name", name)
				_ = errCount.Add(1)
			} else {
				slog.Debug("Downloaded Firefox extension", "name", name, "fname", *fname)
				firefoxResults <- FirefoxResult{Name: name, Fname: *fname}
			}
		}()

	}

	var chromiumWg sync.WaitGroup
	chromiumResults := make(chan ChromiumResult)
	for id := range chromiumToProfiles {
		id := id
		chromiumWg.Add(1)
		go func() {
			defer chromiumWg.Done()
			fname, err := chromium.DownloadExtension(fs, id, extensionsDirChromium)
			if err != nil {
				slog.Error("Failed to download Chromium extension", "id", id, "err", err)
				_ = errCount.Add(1)
			} else {
				slog.Debug("Downloaded Chromium extension", "id", id, "fname", *fname)
				chromiumResults <- ChromiumResult{Id: id, Fname: *fname}
			}
		}()
	}

	var updates atomic.Int32

	var installerWg sync.WaitGroup
	installerWg.Add(1)
	go func() {
		defer installerWg.Done()
		for res := range chromiumResults {
			id := res.Id
			fname := res.Fname
			for _, profile := range chromiumToProfiles[id] {
				slog.Debug("Installing Chromium extension", "id", id, "fname", fname, "profile", profile)
				installed, err := chromium.InstallExtension(fs, fname, profile)
				if err != nil {
					slog.Error("Failed to install Chromium extension", "id", id, "profile", profile, "err", err)
					_ = errCount.Add(1)
					continue
				}
				if installed {
					updates.Add(1)
				}
			}
		}
	}()

	installerWg.Add(1)
	go func() {
		defer installerWg.Done()
		for res := range firefoxResults {
			name := res.Name
			fname := res.Fname
			for _, profile := range firefoxToProfiles[name] {
				slog.Debug("Installing Firefox extension", "name", name, "fname", fname, "profile", profile)
				installed, err := firefox.InstallExtension(fs, fname, profile)
				if err != nil {
					slog.Error("Failed to install Firefox extension", "name", name, "profile", profile, "err", err)
					_ = errCount.Add(1)
					continue
				}
				if installed {
					updates.Add(1)
				}
			}
		}
	}()

	firefoxWg.Wait()
	close(firefoxResults) // this causes the "Installing extension" goroutine to finish

	chromiumWg.Wait()
	close(chromiumResults)

	installerWg.Wait()

	duration := time.Since(start)
	errors := int(errCount.Load())
	slog.Info("extension-downloader finished", "updates", updates.Load(), "duration", duration, "errors", errors)
	return errors
}
