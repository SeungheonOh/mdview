cask "mdiew" do
  arch arm: "aarch64", intel: "x86_64"
  version "0.1.8"
  sha256 arm:   "07d796e63adaf2454acf6b8064bdf47f9c245063f86ad88ad11b5fb8014430ba",
         intel: "28b681e63ffa84c49e99a9266bb6215236aa1d3d281f792904005a7e11a593b1"

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
