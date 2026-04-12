class Macrelay < Formula
  desc "Open-source MCP server for native macOS apps"
  homepage "https://github.com/drbarq/macrelay"
  url "https://github.com/drbarq/macrelay/releases/latest/download/macrelay-macos-universal"
  version "1.0.0"
  sha256 "0019dfc4b32d63c1392aa264aed2253c1e0c2fb09216f8e2cc269bbfb8bb49b5"
  license "MIT"

  depends_on :macos
  depends_on macos: :sonoma

  def install
    bin.install "macrelay-macos-universal" => "macrelay"
  end

  test do
    assert_match "macrelay", shell_output("#{bin}/macrelay --version 2>&1", 0)
  end
end
