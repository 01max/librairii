module Librairii
  class ReadinessGate
    RECOVERY_PATH = "/recovery"
    PASSTHROUGH_PREFIXES = %w[/assets/ /icon. /robots.txt].freeze

    def initialize(app)
      @app = app
    end

    def call(environment)
      request = Rack::Request.new(environment)
      return @app.call(environment) if passthrough?(request.path)

      result = Rails.configuration.x.librairii.readiness.call
      environment["librairii.readiness"] = result

      return @app.call(environment) if result.ready? || request.path == RECOVERY_PATH
      return redirect_to_recovery if request.get? || request.head?

      render_recovery(environment)
    end

    private

    def passthrough?(path)
      PASSTHROUGH_PREFIXES.any? { |prefix| path.start_with?(prefix) }
    end

    def redirect_to_recovery
      [
        302,
        {
          "content-type" => "text/html; charset=utf-8",
          "location" => RECOVERY_PATH,
          "cache-control" => "no-store"
        },
        []
      ]
    end

    def render_recovery(environment)
      recovery_environment = environment.merge(
        "REQUEST_METHOD" => "GET",
        "PATH_INFO" => RECOVERY_PATH,
        "QUERY_STRING" => ""
      )
      _status, headers, body = @app.call(recovery_environment)

      [ 503, headers.merge("cache-control" => "no-store"), body ]
    end
  end
end
