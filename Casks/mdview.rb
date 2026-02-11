cask "mdview" do
  arch arm: "aarch64", intel: "x86_64"
  version "0.1.0"
  sha256 arm:   "PLACEHOLDER",
         intel: "PLACEHOLDER"

  url "https://github.com/SeungheonOh/mdview/releases/download/v#{version}/mdview-#{arch}-apple-darwin.app.zip"
  name "mdview"
  desc "A fast, native macOS markdown viewer"
  homepage "https://github.com/SeungheonOh/mdview"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :monterey"

  app "mdview.app"
end
