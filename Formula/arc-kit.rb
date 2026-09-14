class ArcKit < Formula
  desc "CLI tool for managing coding agent providers, skills, and markets"
  homepage "https://github.com/duoyuli/arc-kit"
  license "MIT"
  version "2026.9.14+1"

  on_arm do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.9.14+1/arc-kit-aarch64-apple-darwin.tar.gz"
    sha256 "b3701ca3dac3fdfd8ab7555c54009889b301c42141d669a7f5801efc92b5357f"
  end

  on_intel do
    url "https://github.com/duoyuli/arc-kit/releases/download/v2026.9.14+1/arc-kit-x86_64-apple-darwin.tar.gz"
    sha256 "eb8d6b1bb3fa938edef955ab9ce33d78d668f3df62a108983faab97d30dcdd52"
  end

  def install
    bin.install "arc"
  end

  test do
    system bin/"arc", "version"
  end
end
