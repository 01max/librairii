class HealthController < ApplicationController
  def show
    readiness = request.env["librairii.readiness"] ||
      Rails.configuration.x.librairii.readiness.call

    if readiness.ready?
      render json: { status: "ok" }
    else
      render json: {
        status: "not_ready",
        issues: readiness.issues.map { |issue| issue.code.to_s }
      }, status: :service_unavailable
    end
  end
end
