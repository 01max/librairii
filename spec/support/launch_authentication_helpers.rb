module LaunchAuthenticationHelpers
  def launch_authorization(secret = Rails.configuration.x.librairii.launch_secret)
    { "HTTP_AUTHORIZATION" => "Bearer #{secret}" }
  end
end

RSpec.configure do |config|
  config.include LaunchAuthenticationHelpers, type: :request
end
