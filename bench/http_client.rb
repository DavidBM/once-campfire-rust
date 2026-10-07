require "net/http"
class BenchmarkHTTPClient
  def initialize(base)
    @base = URI(base)
  end
  def ready?
    Net::HTTP.new(@base.host, @base.port, nil).tap do |http|
      http.open_timeout = 1
      http.read_timeout = 1
      http.max_retries = 0
    end.start { |http| http.get("/up").code == "200" }
  rescue IOError, SystemCallError, Timeout::Error, SocketError
    false
  end
end
