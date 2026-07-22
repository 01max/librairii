require Rails.root.join("lib/librairii/readiness")
require Rails.root.join("lib/librairii/readiness_gate")

application_data = Rails.configuration.x.librairii.application_data
readiness = Librairii::Readiness.new(
  application_data: application_data,
  preparation_error: Rails.configuration.x.librairii.application_data_preparation_error
)

Rails.configuration.x.librairii.readiness = readiness
Rails.configuration.x.librairii.startup_readiness = readiness.call
Rails.application.config.middleware.insert_before(0, Librairii::ReadinessGate)
