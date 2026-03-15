cask "mdiew" do
  arch arm: "aarch64", intel: "x86_64"
  version "0.1.11"
  sha256 arm:   "37c131d43572d3bf21e10e21dd211bc485fefe4edcfbc3d4e4db3b09f48bd604",
         intel: "2ae14b4dd1b3925027764d723d294e887af13b714716c0c5fcf14c7cd5dc4dfb"

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
