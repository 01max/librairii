require "fileutils"
require "pathname"

module Librairii
  class ApplicationData
    DIRECTORY_NAMES = %w[db archives catalog staging trash logs].freeze

    attr_reader :root

    def self.resolve(environment:, application_root:, configured_root: nil)
      root = if configured_root.nil? || configured_root.strip.empty?
        Pathname(application_root).join("tmp", "librairii", environment.to_s)
      else
        configured_root
      end

      new(root)
    end

    def initialize(root)
      @root = Pathname(root).expand_path
    end

    def prepare!
      DIRECTORY_NAMES.each { |name| FileUtils.mkdir_p(path(name)) }
      self
    end

    def path(name)
      unless DIRECTORY_NAMES.include?(name.to_s)
        raise ArgumentError, "unknown application data directory: #{name}"
      end

      root.join(name.to_s)
    end
  end
end
