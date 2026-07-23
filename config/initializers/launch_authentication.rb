require "securerandom"

launch_configuration = Rails.configuration.x.librairii
direct_development = Rails.env.development? && launch_configuration.launch_secret.blank?

launch_configuration.launch_authentication_required = !direct_development
launch_configuration.launch_secret ||= SecureRandom.hex(32) unless direct_development
