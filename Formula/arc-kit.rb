class ArcKit < Formula
  desc "CLI tool for managing coding agent providers, skills, and markets"
  homepage "https://github.com/duoyuli/arc-kit"
  license "MIT"
  version "2026.5.14"

  on_arm do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.5.14/arc-kit-aarch64-apple-darwin.tar.gz"
    sha256 "52863b171d6d124ef3c37d0c1b53594b4b64e7d1e731fb2a6422b36b174a31a9"
  end

  on_intel do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.5.14/arc-kit-x86_64-apple-darwin.tar.gz"
    sha256 "53159699a39acce8970e11f994b7ec877b07c8367633fe32fab1eeff82667cea"
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
