cask "mdiew" do
  arch arm: "aarch64", intel: "x86_64"
  version "0.1.12"
  sha256 arm:   "76b44fdcf3622fe59522aa9c9514b0a8bb076f9df6b746f6788cd6d6dfcef61a",
         intel: "8c6814fbf0552ebf4e022ee42bd08fe5c6c34227fe5aad61d5829a5ca2fb1dd1"

  url "https://github.com/SeungheonOh/mdiew/releases/download/v#{version}/mdiew-#{arch}-apple-darwin.app.zip"
  name "mdiew"
  desc "A fast, native macOS markdown viewer"
  homepage "https://github.com/SeungheonOh/mdiew"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :monterey"

  app "mdiew.app"

  postflight do
    system_command "/usr/bin/xattr", args: ["-cr", "#{appdir}/mdiew.app"]
  end
end
