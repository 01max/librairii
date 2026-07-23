require "digest"
require "ipaddr"
require "openssl"

module Librairii
  class LaunchAuthentication
    COOKIE_NAME = "_librairii_launch"
    QUERY_PARAMETER = "launch_secret"

    def initialize(app)
      @app = app
    end

    def call(environment)
      request = Rack::Request.new(environment)
      return forbidden unless loopback?(environment["REMOTE_ADDR"])
      return @app.call(environment) unless authentication_required?

      if bootstrap?(request)
        return bootstrap_response(request)
      end

      return forbidden unless authenticated?(request)

      @app.call(environment)
    end

    private

    def authentication_required?
      Rails.configuration.x.librairii.launch_authentication_required != false
    end

    def launch_secret
      Rails.configuration.x.librairii.launch_secret.to_s
    end

    def loopback?(address)
      IPAddr.new(address).loopback?
    rescue IPAddr::InvalidAddressError
      false
    end

    def bootstrap?(request)
      request.get? && valid_secret?(request.GET[QUERY_PARAMETER])
    end

    def authenticated?(request)
      valid_secret?(bearer_token(request)) || valid_cookie?(request.cookies[COOKIE_NAME])
    end

    def bearer_token(request)
      scheme, token = request.get_header("HTTP_AUTHORIZATION").to_s.split(" ", 2)
      token if scheme&.casecmp?("Bearer")
    end

    def valid_secret?(candidate)
      return false if candidate.nil? || candidate.empty? || launch_secret.empty?

      ActiveSupport::SecurityUtils.secure_compare(
        Digest::SHA256.hexdigest(candidate),
        Digest::SHA256.hexdigest(launch_secret)
      )
    end

    def valid_cookie?(candidate)
      return false if candidate.nil? || candidate.empty?

      ActiveSupport::SecurityUtils.secure_compare(candidate, cookie_value)
    end

    def cookie_value
      OpenSSL::HMAC.hexdigest("SHA256", launch_secret, "librairii-launch-cookie")
    end

    def bootstrap_response(request)
      query = request.GET.except(QUERY_PARAMETER)
      location = request.path
      location = "#{location}?#{Rack::Utils.build_nested_query(query)}" if query.any?

      [
        303,
        {
          "content-type" => "text/html; charset=utf-8",
          "location" => location,
          "set-cookie" => "#{COOKIE_NAME}=#{cookie_value}; Path=/; HttpOnly; SameSite=Strict",
          "cache-control" => "no-store"
        },
        []
      ]
    end

    def forbidden
      body = "Local launch authentication required."
      [
        403,
        {
          "content-type" => "text/plain; charset=utf-8",
          "content-length" => body.bytesize.to_s,
          "cache-control" => "no-store"
        },
        [ body ]
      ]
    end
  end
end
