require "fileutils"
require "open3"
require "pathname"
require "spec_helper"
require "tmpdir"

RSpec.describe "script/tauri" do
  project_root = Pathname(__dir__).join("../..").expand_path

  def write_executable(path, contents)
    File.write(path, contents)
    FileUtils.chmod(0o755, path)
  end

  it "finds Cargo through rustup when the shell PATH has no Cargo proxy" do
    Dir.mktmpdir("librairii-tauri-launcher") do |temporary_directory|
      temporary_root = Pathname(temporary_directory)
      shell_bin = temporary_root.join("shell-bin").tap(&:mkpath)
      toolchain_bin = temporary_root.join("toolchain-bin").tap(&:mkpath)
      fake_tauri = temporary_root.join("tauri")
      fake_cargo = toolchain_bin.join("cargo")

      write_executable(shell_bin.join("rustup"), <<~SH)
        #!/bin/sh
        test "$1" = "which" && test "$2" = "cargo"
        printf '%s\n' "$FAKE_CARGO"
      SH
      write_executable(fake_cargo, <<~SH)
        #!/bin/sh
        test "$1" = "metadata"
        printf 'cargo metadata reached\n'
      SH
      write_executable(fake_tauri, <<~SH)
        #!/bin/sh
        cargo metadata --no-deps --format-version 1
      SH

      environment = {
        "FAKE_CARGO" => fake_cargo.to_s,
        "HOME" => temporary_root.to_s,
        "LIBRAIRII_TAURI_CLI" => fake_tauri.to_s,
        "PATH" => "#{shell_bin}:/usr/bin:/bin"
      }
      stdout, stderr, status = Open3.capture3(
        environment,
        project_root.join("script/tauri").to_s,
        "build",
        unsetenv_others: true
      )

      expect(status).to be_success, stderr
      expect(stdout).to include("cargo metadata reached")
    end
  end
end
