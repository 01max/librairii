require "securerandom"

module Librairii
  class Readiness
    Issue = Data.define(:code, :message)
    Result = Data.define(:issues) do
      def ready?
        issues.empty?
      end
    end

    class FileProbe
      def initialize(application_data)
        @application_data = application_data
      end

      def call
        directories = ApplicationData::DIRECTORY_NAMES.map do |name|
          @application_data.path(name)
        end
        return false unless directories.all?(&:directory?)

        probe_path = @application_data.path("staging")
          .join(".readiness-#{Process.pid}-#{SecureRandom.hex(6)}")

        File.open(probe_path, File::WRONLY | File::CREAT | File::EXCL, 0o600) do |file|
          file.write("ready")
          file.flush
          file.fsync
        end
        File.delete(probe_path)
        true
      ensure
        File.delete(probe_path) if defined?(probe_path) && probe_path&.exist?
      end
    end

    class DatabaseProbe
      def call
        table_name = "librairii_readiness_#{SecureRandom.hex(6)}"
        connection = ActiveRecord::Base.connection

        connection.transaction(requires_new: true) do
          connection.create_table(table_name, id: false) do |table|
            table.string :probe, null: false
          end
          connection.execute("INSERT INTO #{connection.quote_table_name(table_name)} (probe) VALUES ('ready')")
          raise ActiveRecord::Rollback
        end

        true
      ensure
        connection&.drop_table(table_name, if_exists: true)
      end
    end

    def initialize(application_data:, preparation_error: nil, file_probe: nil, database_probe: nil)
      @preparation_error = preparation_error
      @file_probe = file_probe || FileProbe.new(application_data)
      @database_probe = database_probe || DatabaseProbe.new
    end

    def call
      issues = []
      issues << issue(:data_root, @preparation_error) if @preparation_error
      issues << issue(:data_root) unless @preparation_error || probe(@file_probe)
      issues << issue(:database) unless probe(@database_probe)
      Result.new(issues: issues)
    end

    private

    def probe(probe)
      probe.call
    rescue StandardError
      false
    end

    def issue(code, error = nil)
      message = case code
      when :data_root
        "The application data folder is not writable."
      when :database
        "The SQLite database is not writable."
      end
      message = "#{message} #{error.message}" if error

      Issue.new(code: code, message: message)
    end
  end
end
