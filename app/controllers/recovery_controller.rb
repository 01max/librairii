class RecoveryController < ApplicationController
  def show
    @readiness = request.env["librairii.readiness"] ||
      Rails.configuration.x.librairii.readiness.call

    render status: @readiness.ready? ? :ok : :service_unavailable
  end
end
