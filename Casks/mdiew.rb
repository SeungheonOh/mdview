cask "mdiew" do
  arch arm: "aarch64", intel: "x86_64"
  version "0.1.10"
  sha256 arm:   "b9422d0e0b1b16154919729c19137c9d8895121c5adf8522664b109b16332a7b",
         intel: "bbbb51990fb2021702ce0bb1553dbb43196c0bf721a166dfa66afb271fcd60b8"

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
