require "fileutils"
require "open3"
require "pathname"
require "spec_helper"
require "tmpdir"

RSpec.describe "bin/build" do
  project_root = Pathname(__dir__).join("../..").expand_path

  it "builds the packaged runtime before the Tauri application" do
    Dir.mktmpdir("librairii-build-command") do |temporary_directory|
      temporary_root = Pathname(temporary_directory)
      fake_bin = temporary_root.join("bin").tap(&:mkpath)
      invocation_log = temporary_root.join("npm-invocations")
      fake_npm = fake_bin.join("npm")
      File.write(fake_npm, <<~SH)
        #!/bin/sh
        printf '%s\n' "$*" >> "$BUILD_INVOCATION_LOG"
      SH
      FileUtils.chmod(0o755, fake_npm)

      environment = {
        "BUILD_INVOCATION_LOG" => invocation_log.to_s,
        "PATH" => "#{fake_bin}:/usr/bin:/bin"
      }
      _stdout, stderr, status = Open3.capture3(
        environment,
        project_root.join("bin/build").to_s,
        unsetenv_others: true
      )

      expect(status).to be_success, stderr
      expect(invocation_log.readlines(chomp: true)).to eq([
        "run package:runtime",
        "run tauri:build"
      ])
    end
  end
end
