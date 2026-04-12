cask "macrelay" do
  version "1.1.1"
  sha256 "c8e07889455423b522a44579dc8d02ab2f7ef964e0c806834ca2beb2485c2cb8"

  url "https://github.com/drbarq/macrelay/releases/download/v#{version}/MacRelay.zip"
  name "MacRelay"
  desc "Open-source MCP server for native macOS apps — with menu bar manager"
  homepage "https://github.com/drbarq/macrelay"

  depends_on macos: ">= :sonoma"

  app "MacRelay.app"

  postflight do
    # Strip quarantine flag so Gatekeeper doesn't block unsigned app
    system "xattr", "-cr", "#{appdir}/MacRelay.app"
    # Launch the app so the menu bar icon appears
    system "open", "#{appdir}/MacRelay.app"
  end

  uninstall quit: "com.macrelay.app"

  zap trash: [
    "~/Library/Application Support/MacRelay",
    "~/Library/LaunchAgents/com.macrelay.menubar.plist",
    "~/Library/Application Support/Claude/Claude Extensions/com.macrelay.app",
    "~/Library/Application Support/Claude/Claude Extensions Settings/com.macrelay.app.json",
  ]
end
