require "securerandom"

Rails.configuration.x.librairii.launch_secret ||= SecureRandom.hex(32)
