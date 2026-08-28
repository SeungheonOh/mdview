cask "mdiew" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.1.13"
  sha256 arm:   "e3cc9e3723fd521063e67d91e37668ea1a593aa46235e46f8c0c774492d41cff",
         intel: "b4d8e96e3608f5c3068b4803b842ce0e57c390187f293b8122fac247dafe70cc"

  url "https://github.com/SeungheonOh/mdiew/releases/download/v#{version}/mdiew-#{arch}-apple-darwin.app.zip"
  name "mdiew"
  desc "Fast native Markdown viewer"
  homepage "https://github.com/SeungheonOh/mdiew"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: :monterey

  app "mdiew.app"

  postflight do
    system_command "/usr/bin/xattr", args: ["-cr", "#{appdir}/mdiew.app"]
  end
end
