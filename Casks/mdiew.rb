cask "mdiew" do
  arch arm: "aarch64", intel: "x86_64"
  version "0.1.6"
  sha256 arm:   "45c59bdbec213d268b8142acd1b6bb995b6c835316c16af0519fab47f3d6ab1a",
         intel: "a8abadc65686c8baeef9449811b6cb33241000b21426e60ca34cbbc4bf3f5333"

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
end
